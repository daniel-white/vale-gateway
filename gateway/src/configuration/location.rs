use typed_builder::TypedBuilder;
use vg_core::configuration::watch::{channel, ConfigurationSender, ConfigurationWatch};
use vg_core::net::topology::TopologyLocation;

#[derive(TypedBuilder)]
pub struct CurrentLocationConfigurator {
    tx: ConfigurationSender<TopologyLocation>,
    rx: ConfigurationWatch<TopologyLocation>
}

impl CurrentLocationConfigurator {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self::builder()
            .tx(tx)
            .rx(rx)
            .build()
    }

    pub fn current_location(&self) -> ConfigurationWatch<TopologyLocation> {
        self.rx.clone()
    }
    
    pub fn start(self) -> ConfigurationSender<TopologyLocation> {
        self.tx
    }
}

