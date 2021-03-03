use crate::bootstrap_agent::bootstrap_agent::BootstrapAgent;
use crate::bootstrap_agent::http_entrypoints;
use crate::bootstrap_agent::Config;
use std::sync::Arc;

/// Wraps a [BootstrapAgent] object, and provides helper methods for exposing it
/// via an HTTP interface.
pub struct Server {
    bootstrap_agent: Arc<BootstrapAgent>,
    http_server: dropshot::HttpServer<Arc<BootstrapAgent>>,
}

impl Server {
    pub async fn start(config: &Config) -> Result<Self, String> {
        let log = config
            .log
            .to_logger("bootstrap-agent")
            .map_err(|message| format!("initializing logger: {}", message))?;
        info!(log, "setting up bootstrap agent server");

        let ba_log = log.new(o!(
            "component" => "BootstrapAgent",
            "server" => config.id.clone().to_string()
        ));
        let bootstrap_agent = Arc::new(BootstrapAgent::new(ba_log));

        let ba = Arc::clone(&bootstrap_agent);
        let dropshot_log = log.new(o!("component" => "dropshot"));
        let http_server = dropshot::HttpServerStarter::new(
            &config.dropshot,
            http_entrypoints::ba_api(),
            ba,
            &dropshot_log,
        )
        .map_err(|error| format!("initializing server: {}", error))?
        .start();

        let server = Server { bootstrap_agent, http_server };

        // Initialize the bootstrap agent *after* the server has started.
        // This ordering allows the bootstrap agent to communicate with
        // other bootstrap agents on the rack during the initialization
        // process.
        if let Err(e) = server.bootstrap_agent.initialize(vec![]).await {
            let _ = server.close().await;
            return Err(e.to_string());
        }

        Ok(server)
    }

    pub async fn wait_for_finish(self) -> Result<(), String> {
        self.http_server.await
    }

    pub async fn close(self) -> Result<(), String> {
        self.http_server.close().await
    }
}
