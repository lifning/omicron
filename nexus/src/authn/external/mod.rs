// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Authentication for requests to the external HTTP API

use crate::authn;
use async_trait::async_trait;
use authn::Reason;

pub mod cookies;
pub mod session_cookie;
pub mod spoof;

/// Authenticates incoming HTTP requests using schemes intended for use by the
/// external API
pub struct Authenticator<T> {
    allowed_schemes: Vec<Box<dyn HttpAuthnScheme<T>>>,
}

impl<T> Authenticator<T>
where
    T: Send + Sync + 'static,
{
    /// Build a new authentiator that allows only the specified schemes
    pub fn new(
        allowed_schemes: Vec<Box<dyn HttpAuthnScheme<T>>>,
    ) -> Authenticator<T> {
        Authenticator { allowed_schemes }
    }

    /// Authenticate an incoming HTTP request
    // TODO-openapi: At some point, the authentication headers need to get into
    // the OpenAPI spec.  We probably don't want to have every endpoint function
    // accept them via an extractor, though.
    pub async fn authn_request(
        &self,
        rqctx: &dropshot::RequestContext<T>,
    ) -> Result<authn::Context, authn::Error> {
        let log = &rqctx.log;
        let request = &rqctx.request.lock().await;
        let ctx = rqctx.context();
        let result = self.authn_request_generic(ctx, log, request).await;
        trace!(log, "authn result: {:?}", result);
        result
    }

    /// Authenticate an incoming HTTP request (dropshot-agnostic)
    pub async fn authn_request_generic(
        &self,
        ctx: &T,
        log: &slog::Logger,
        request: &http::Request<hyper::Body>,
    ) -> Result<authn::Context, authn::Error> {
        // For debuggability, keep track of the schemes that we've tried.
        let mut schemes_tried = Vec::with_capacity(self.allowed_schemes.len());
        for scheme_impl in &self.allowed_schemes {
            let scheme_name = scheme_impl.name();
            trace!(log, "authn: trying {:?}", scheme_name);
            schemes_tried.push(scheme_name);
            let result = scheme_impl.authn(ctx, log, &request).await;
            match result {
                // TODO-security If the user explicitly failed one
                // authentication scheme (i.e., a signature that didn't match,
                // NOT that they simply didn't try), should we try the others
                // instead of returning the failure here?
                SchemeResult::Failed(reason) => {
                    return Err(authn::Error { reason, schemes_tried })
                }
                SchemeResult::Authenticated(details) => {
                    return Ok(authn::Context {
                        kind: authn::Kind::Authenticated(details),
                        schemes_tried,
                    })
                }
                SchemeResult::NotRequested => (),
            }
        }

        Ok(authn::Context { kind: authn::Kind::Unauthenticated, schemes_tried })
    }
}

/// Implements a particular HTTP authentication scheme
#[async_trait]
pub trait HttpAuthnScheme<T>: std::fmt::Debug + Send + Sync + 'static
where
    T: Send + Sync + 'static,
{
    /// Returns the (unique) name for this scheme (for observability)
    fn name(&self) -> authn::SchemeName;

    /// Locate credentials in the HTTP request and attempt to verify them
    async fn authn(
        &self,
        ctx: &T,
        log: &slog::Logger,
        request: &http::Request<hyper::Body>,
    ) -> SchemeResult;
}

/// Result returned by each authentication scheme when trying to authenticate a
/// request
#[derive(Debug)]
pub enum SchemeResult {
    /// The client is not trying to use this authn scheme
    NotRequested,
    /// The client successfully authenticated
    Authenticated(super::Details),
    /// The client tried and failed to authenticate
    Failed(Reason),
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::anyhow;
    use std::sync::atomic::AtomicU8;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    /// HttpAuthnScheme that we can precisely control
    #[derive(Debug)]
    struct GruntScheme {
        /// unique name for this grunt
        name: authn::SchemeName,

        /// Specifies what to do with the next authn request that we get
        ///
        /// See "SKIP", "OK", and "FAIL" below.
        next: Arc<AtomicU8>,

        /// number of times we've been asked to authn a request
        nattempts: Arc<AtomicU8>,

        /// actor to use when authenticated
        actor: authn::Actor,
    }

    // Values of the "next" bool
    const SKIP: u8 = 0;
    const OK: u8 = 1;
    const FAIL: u8 = 2;

    #[async_trait]
    impl HttpAuthnScheme<()> for GruntScheme {
        fn name(&self) -> authn::SchemeName {
            self.name
        }

        async fn authn(
            &self,
            _ctx: &(),
            _log: &slog::Logger,
            _request: &http::Request<hyper::Body>,
        ) -> SchemeResult {
            self.nattempts.fetch_add(1, Ordering::SeqCst);
            match self.next.load(Ordering::SeqCst) {
                SKIP => SchemeResult::NotRequested,
                OK => SchemeResult::Authenticated(authn::Details {
                    actor: self.actor,
                }),
                FAIL => SchemeResult::Failed(Reason::BadCredentials {
                    actor: self.actor,
                    source: anyhow!("grunt error"),
                }),
                _ => panic!("unrecognized grunt instruction"),
            }
        }
    }

    #[tokio::test]
    async fn test_authn_sequence() {
        // This test verifies the basic behavior of Authenticator by setting up
        // a chain of two authn schemes that we can control and measure.  We
        // will verify:
        //
        // - when the first scheme returns "authenticated" or an error, we take
        //   its result and don't even consult the second scheme
        // - when the first scheme returns "unauthenticated", we consult the
        //   second scheme and use its result
        // - when both schemes return "unauthenticated", we get back an
        //   unauthenticated context

        // Set up the Authenticator with two GruntSchemes.
        let flag1 = Arc::new(AtomicU8::new(SKIP));
        let count1 = Arc::new(AtomicU8::new(0));
        let mut expected_count1 = 0;
        let name1 = authn::SchemeName("grunt1");
        let actor1 = authn::Actor(
            "1c91bab2-4841-669f-cc32-de80da5bbf39".parse().unwrap(),
        );
        let grunt1 = Box::new(GruntScheme {
            name: name1,
            next: Arc::clone(&flag1),
            nattempts: Arc::clone(&count1),
            actor: actor1,
        }) as Box<dyn HttpAuthnScheme<()>>;

        let flag2 = Arc::new(AtomicU8::new(SKIP));
        let count2 = Arc::new(AtomicU8::new(0));
        let mut expected_count2 = 0;
        let name2 = authn::SchemeName("grunt2");
        let actor2 = authn::Actor(
            "799684af-533a-cb66-b5ac-ab55a791d5ef".parse().unwrap(),
        );
        let grunt2 = Box::new(GruntScheme {
            name: name2,
            next: Arc::clone(&flag2),
            nattempts: Arc::clone(&count2),
            actor: actor2,
        }) as Box<dyn HttpAuthnScheme<()>>;

        let authn = Authenticator::new(vec![grunt1, grunt2]);
        let request = http::Request::builder()
            .uri("/unused")
            .body(hyper::Body::empty())
            .unwrap();

        let log = slog::Logger::root(slog::Discard, o!());

        // With this initial state, both grunts will report that authn was not
        // requested.  We should wind up with an unauthenticated context with
        // both grunts having been consulted.
        let ctx = authn
            .authn_request_generic(&(), &log, &request)
            .await
            .expect("expected authn to succeed");
        expected_count1 += 1;
        expected_count2 += 1;
        assert_eq!(ctx.schemes_tried(), &[name1, name2]);
        assert_eq!(ctx.actor(), None);
        assert_eq!(expected_count1, count1.load(Ordering::SeqCst));
        assert_eq!(expected_count2, count2.load(Ordering::SeqCst));

        // Now let's configure grunt1 to authenticate the user.  We should get
        // back an authenticated context with grunt1's actor id.  grunt2 should
        // not be consulted.
        flag1.store(OK, Ordering::SeqCst);
        let ctx = authn
            .authn_request_generic(&(), &log, &request)
            .await
            .expect("expected authn to succeed");
        expected_count1 += 1;
        assert_eq!(ctx.schemes_tried(), &[name1]);
        assert_eq!(ctx.actor(), Some(&actor1));
        assert_eq!(expected_count1, count1.load(Ordering::SeqCst));
        assert_eq!(expected_count2, count2.load(Ordering::SeqCst));

        // Now let's configure grunt1 to fail authentication.  We should get
        // back an error.  grunt2 should not be consulted.
        flag1.store(FAIL, Ordering::SeqCst);
        let error = authn
            .authn_request_generic(&(), &log, &request)
            .await
            .expect_err("expected authn to fail");
        expected_count1 += 1;
        assert_eq!(
            error.to_string(),
            "authentication failed (tried schemes: [SchemeName(\"grunt1\")])"
        );
        assert_eq!(expected_count1, count1.load(Ordering::SeqCst));
        assert_eq!(expected_count2, count2.load(Ordering::SeqCst));

        // We've now verified that grunt2 is not consulted unless grunt1 reports
        // that authentication was not requested.  Let's configure grunt1 to do
        // exactly that and have grunt2 successfully authenticate.
        flag1.store(SKIP, Ordering::SeqCst);
        flag2.store(OK, Ordering::SeqCst);
        let ctx = authn
            .authn_request_generic(&(), &log, &request)
            .await
            .expect("expected authn to succeed");
        expected_count1 += 1;
        expected_count2 += 1;
        assert_eq!(ctx.schemes_tried(), &[name1, name2]);
        assert_eq!(ctx.actor(), Some(&actor2));
        assert_eq!(expected_count1, count1.load(Ordering::SeqCst));
        assert_eq!(expected_count2, count2.load(Ordering::SeqCst));

        // Now configure grunt2 to fail.
        flag2.store(FAIL, Ordering::SeqCst);
        expected_count1 += 1;
        expected_count2 += 1;
        let error = authn
            .authn_request_generic(&(), &log, &request)
            .await
            .expect_err("expected authn to fail");
        assert_eq!(
            error.to_string(),
            "authentication failed (tried schemes: \
            [SchemeName(\"grunt1\"), SchemeName(\"grunt2\")])"
        );
        assert_eq!(expected_count1, count1.load(Ordering::SeqCst));
        assert_eq!(expected_count2, count2.load(Ordering::SeqCst));
    }
}
