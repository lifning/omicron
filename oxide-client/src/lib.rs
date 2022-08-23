// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Interface for making API requests to the Oxide control plane.

progenitor::generate_api!(
    spec = "../openapi/nexus.json",
    interface = Builder,
    tags = Separate,
);
/*
use progenitor::progenitor_client::ByteStream;
pub struct Client {
    pub(crate) baseurl: String,
    pub(crate) client: reqwest::Client,
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::clone::Clone for Client {
    #[inline]
    fn clone(&self) -> Client {
        match *self {
            Client {
                baseurl: ref __self_0_0,
                client: ref __self_0_1,
            } => Client {
                baseurl: ::core::clone::Clone::clone(&(*__self_0_0)),
                client: ::core::clone::Clone::clone(&(*__self_0_1)),
            },
        }
    }
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::fmt::Debug for Client {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match *self {
            Client {
                baseurl: ref __self_0_0,
                client: ref __self_0_1,
            } => {
                let debug_trait_builder = &mut ::core::fmt::Formatter::debug_struct(f, "Client");
                let _ = ::core::fmt::DebugStruct::field(
                    debug_trait_builder,
                    "baseurl",
                    &&(*__self_0_0),
                );
                let _ =
                    ::core::fmt::DebugStruct::field(debug_trait_builder, "client", &&(*__self_0_1));
                ::core::fmt::DebugStruct::finish(debug_trait_builder)
            }
        }
    }
}
impl Client {
    pub fn new(baseurl: &str) -> Self {
        let dur = std::time::Duration::from_secs(15);
        let client = reqwest::ClientBuilder::new()
            .connect_timeout(dur)
            .timeout(dur)
            .build()
            .unwrap();
        Self::new_with_client(baseurl, client)
    }
    pub fn new_with_client(baseurl: &str, client: reqwest::Client) -> Self {
        Self {
            baseurl: baseurl.to_string(),
            client,
        }
    }
    pub fn baseurl(&self) -> &String {
        &self.baseurl
    }
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
}
///Builder for [`ClientInstancesExt::instance_serial_console_2`]
///
///[`ClientInstancesExt::instance_serial_console_2`]: super::ClientInstancesExt::instance_serial_console_2
pub struct InstanceSerialConsole2<'a> {
    client: &'a self::Client,
    organization_name: Result<types::Name, String>,
    project_name: Result<types::Name, String>,
    instance_name: Result<types::Name, String>,
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<'a> ::core::fmt::Debug for InstanceSerialConsole2<'a> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        match *self {
            InstanceSerialConsole2 {
                client: ref __self_0_0,
                organization_name: ref __self_0_1,
                project_name: ref __self_0_2,
                instance_name: ref __self_0_3,
            } => {
                let debug_trait_builder =
                    &mut ::core::fmt::Formatter::debug_struct(f, "InstanceSerialConsole2");
                let _ = ::core::fmt::DebugStruct::field(
                    debug_trait_builder,
                    "client",
                    &&(*__self_0_0),
                );
                let _ = ::core::fmt::DebugStruct::field(
                    debug_trait_builder,
                    "organization_name",
                    &&(*__self_0_1),
                );
                let _ = ::core::fmt::DebugStruct::field(
                    debug_trait_builder,
                    "project_name",
                    &&(*__self_0_2),
                );
                let _ = ::core::fmt::DebugStruct::field(
                    debug_trait_builder,
                    "instance_name",
                    &&(*__self_0_3),
                );
                ::core::fmt::DebugStruct::finish(debug_trait_builder)
            }
        }
    }
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<'a> ::core::clone::Clone for InstanceSerialConsole2<'a> {
    #[inline]
    fn clone(&self) -> InstanceSerialConsole2<'a> {
        match *self {
            InstanceSerialConsole2 {
                client: ref __self_0_0,
                organization_name: ref __self_0_1,
                project_name: ref __self_0_2,
                instance_name: ref __self_0_3,
            } => InstanceSerialConsole2 {
                client: ::core::clone::Clone::clone(&(*__self_0_0)),
                organization_name: ::core::clone::Clone::clone(&(*__self_0_1)),
                project_name: ::core::clone::Clone::clone(&(*__self_0_2)),
                instance_name: ::core::clone::Clone::clone(&(*__self_0_3)),
            },
        }
    }
}
impl<'a> InstanceSerialConsole2<'a> {
    pub fn new(client: &'a self::Client) -> Self {
        Self {
            client,
            organization_name: Err("organization_name was not initialized".to_string()),
            project_name: Err("project_name was not initialized".to_string()),
            instance_name: Err("instance_name was not initialized".to_string()),
        }
    }
    pub fn organization_name<V>(mut self, value: V) -> Self
        where
            V: TryInto<types::Name>,
    {
        self.organization_name = value
            .try_into()
            .map_err(|_| "conversion to `Name` for organization_name failed".to_string());
        self
    }
    pub fn project_name<V>(mut self, value: V) -> Self
        where
            V: TryInto<types::Name>,
    {
        self.project_name = value
            .try_into()
            .map_err(|_| "conversion to `Name` for project_name failed".to_string());
        self
    }
    pub fn instance_name<V>(mut self, value: V) -> Self
        where
            V: TryInto<types::Name>,
    {
        self.instance_name = value
            .try_into()
            .map_err(|_| "conversion to `Name` for instance_name failed".to_string());
        self
    }
    ///Sends a `GET` request to `/organizations/{organization_name}/projects/{project_name}/instances/{instance_name}/serial-console-2`
    pub async fn send(self) -> Result<ResponseValue<ByteStream>, Error<ByteStream>> {
        let Self {
            client,
            organization_name,
            project_name,
            instance_name,
        } = self;
        let organization_name = organization_name.map_err(Error::InvalidRequest)?;
        let project_name = project_name.map_err(Error::InvalidRequest)?;
        let instance_name = instance_name.map_err(Error::InvalidRequest)?;
        let url = {
            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                &[
                    "",
                    "/organizations/",
                    "/projects/",
                    "/instances/",
                    "/serial-console-2",
                ],
                &[
                    ::core::fmt::ArgumentV1::new_display(&client.baseurl),
                    ::core::fmt::ArgumentV1::new_display(&encode_path(
                        &organization_name.to_string(),
                    )),
                    ::core::fmt::ArgumentV1::new_display(&encode_path(
                        &project_name.to_string(),
                    )),
                    ::core::fmt::ArgumentV1::new_display(&encode_path(
                        &instance_name.to_string(),
                    )),
                ],
            ));
            res
        };
        let request = client.client.get(url).build()?;
        request.url().
        let result = client.client.execute(request).await;
        let response = result?;
        match response.status().as_u16() {
            200..=299 => Ok(ResponseValue::stream(response)),
            _ => Err(Error::ErrorResponse(ResponseValue::stream(response))),
        }
    }
}
*/