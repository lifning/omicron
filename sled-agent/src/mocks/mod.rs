// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Mock structures for testing.

use mockall::mock;
use nexus_client::types::{
    DatasetPutRequest, DatasetPutResponse, InstanceRuntimeState,
    SledAgentStartupInfo, ZpoolPutRequest, ZpoolPutResponse,
};
use progenitor::progenitor_client::Error;
use slog::Logger;
use uuid::Uuid;

mock! {
    pub NexusClient {
        pub fn new(server_addr: &str, log: Logger) -> Self;
        pub async fn cpapi_sled_agents_post(
            &self,
            id: &Uuid,
            info: &SledAgentStartupInfo,
        ) -> Result<(), Error<()>>;
        pub async fn cpapi_instances_put(
            &self,
            id: &Uuid,
            new_runtime_state: &InstanceRuntimeState,
        ) -> Result<(), Error<()>>;
        pub async fn zpool_put(
            &self,
            sled_id: &Uuid,
            zpool_id: &Uuid,
            info: &ZpoolPutRequest,
        ) -> Result<ZpoolPutResponse, Error<()>>;
        pub async fn dataset_put(
            &self,
            zpool_id: &Uuid,
            dataset_id: &Uuid,
            info: &DatasetPutRequest,
        ) -> Result<DatasetPutResponse, Error<()>>;
    }
}
