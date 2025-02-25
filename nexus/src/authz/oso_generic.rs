// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Oso integration

use super::actor::AnyActor;
use super::actor::AuthenticatedActor;
use super::api_resources::Fleet;
use super::api_resources::FleetChild;
use super::api_resources::Organization;
use super::api_resources::Project;
use super::api_resources::ProjectChild;
use super::context::AuthorizedResource;
use super::roles::RoleSet;
use super::Authz;
use crate::authn;
use crate::context::OpContext;
use crate::db::DataStore;
use anyhow::Context;
use futures::future::BoxFuture;
use futures::FutureExt;
use omicron_common::api::external::Error;
use oso::Oso;
use oso::PolarClass;
use std::fmt;

/// Polar configuration describing control plane authorization rules
pub const OMICRON_AUTHZ_CONFIG: &str = include_str!("omicron.polar");

/// Returns an Oso handle suitable for authorizing using Omicron's authorization
/// rules
pub fn make_omicron_oso() -> Result<Oso, anyhow::Error> {
    let mut oso = Oso::new();
    // TODO-cleanup There is a lot of boilerplate in the definitions of these
    // structures as they relate to Polar.  For example, most of them impl Eq
    // and PartialEq, and the corresponding PolarClass should have an equality
    // impl as well.  The resources all have a "has_role" Polar method that all
    // do the same thing.  It'd be nice to find a way to commonize these.  A
    // macro might help.
    let classes = [
        Action::get_polar_class(),
        AnyActor::get_polar_class(),
        AuthenticatedActor::get_polar_class(),
        Database::get_polar_class(),
        Fleet::get_polar_class(),
        Organization::get_polar_class(),
        Project::get_polar_class(),
        ProjectChild::get_polar_class(),
        FleetChild::get_polar_class(),
    ];
    for c in classes {
        oso.register_class(c).context("registering class")?;
    }
    oso.load_str(OMICRON_AUTHZ_CONFIG)
        .context("loading built-in Polar (Oso) config")?;
    Ok(oso)
}

/// Describes an action being authorized
///
/// There's currently just one enum of Actions for all of Omicron.  We expect
/// most objects to support mostly the same set of actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Query, // only used for [`Database`]
    Read,
    Modify,
    Delete,
    ListChildren,
    CreateChild,
}

impl oso::PolarClass for Action {
    fn get_polar_class_builder() -> oso::ClassBuilder<Self> {
        oso::Class::builder()
            .with_equality_check()
            .add_method("to_perm", |a: &Action| Perm::from(a).to_string())
    }
}

/// A permission used in the Polar configuration
///
/// An authorization request starts by asking whether an actor can take some
/// _action_ on a resource.  Most of the policy is written in terms of
/// traditional RBAC-style _permissions_.  This type is used to help translate
/// from [`Action`] to permission.
///
/// Note that Polar appears to require that all permissions be strings.  So in
/// practice, the [`Action`] is converted to a [`Perm`] only for long enough to
/// convert that to a string.  Still, having a separate type here ensures that
/// not _any_ old string can be used as a permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Perm {
    Query, // Only for [`Database`]
    Read,
    Modify,
    ListChildren,
    CreateChild,
}

impl From<&Action> for Perm {
    fn from(a: &Action) -> Self {
        match a {
            Action::Query => Perm::Query,
            Action::Read => Perm::Read,
            Action::Modify => Perm::Modify,
            Action::Delete => Perm::Modify,
            Action::ListChildren => Perm::ListChildren,
            Action::CreateChild => Perm::CreateChild,
        }
    }
}

impl fmt::Display for Perm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // This implementation MUST be kept in sync with the Polar configuration
        // for Omicron, which uses literal strings for permissions.
        f.write_str(match self {
            Perm::Query => "query",
            Perm::Read => "read",
            Perm::Modify => "modify",
            Perm::ListChildren => "list_children",
            Perm::CreateChild => "create_child",
        })
    }
}

// Non-API resources that we want to protect with authorization

/// Represents the database itself to Polar
///
/// This exists so that we can have roles with no access to the database at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Database;
/// Singleton representing the [`Database`] itself for authz purposes
pub const DATABASE: Database = Database;

impl oso::PolarClass for Database {
    fn get_polar_class_builder() -> oso::ClassBuilder<Self> {
        oso::Class::builder().add_method(
            "has_role",
            |_d: &Database, _actor: AuthenticatedActor, _role: String| {
                // There is an explicit rule in the Oso policy granting the
                // appropriate roles on "Database" to the appropriate actors.
                // We don't need to grant anything extra here.
                false
            },
        )
    }
}

impl AuthorizedResource for Database {
    fn load_roles<'a, 'b, 'c, 'd, 'e, 'f>(
        &'a self,
        _: &'b OpContext,
        _: &'c DataStore,
        _: &'d authn::Context,
        _: &'e mut RoleSet,
    ) -> BoxFuture<'f, Result<(), Error>>
    where
        'a: 'f,
        'b: 'f,
        'c: 'f,
        'd: 'f,
        'e: 'f,
    {
        // We don't use (database) roles to grant access to the database.  The
        // role assignment is hardcoded for all authenticated users.  See the
        // "has_role" Polar method above.
        //
        // Instead of this, we could modify this function to insert into
        // `RoleSet` the "database user" role.  However, this doesn't fit into
        // the type signature of roles supported by RoleSet.  RoleSet is really
        // for roles on database objects -- it assumes they have a ResourceType
        // and id, neither of which is true for `Database`.
        futures::future::ready(Ok(())).boxed()
    }

    fn on_unauthorized(
        &self,
        _: &Authz,
        error: Error,
        _: AnyActor,
        _: Action,
    ) -> Error {
        error
    }
}
