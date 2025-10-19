use typed_builder::TypedBuilder;
use vg_core::net::topology::TopologyLocation;
use vg_core::sync::arc_watch::{Receiver, Sender, channel};

#[derive(TypedBuilder)]
pub struct CurrentLocationConfigurator {
    tx: Sender<TopologyLocation>,
    rx: Receiver<TopologyLocation>,
}

impl CurrentLocationConfigurator {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self::builder().tx(tx).rx(rx).build()
    }

    pub fn current_location(&self) -> Receiver<TopologyLocation> {
        self.rx.clone()
    }

    pub fn start(self) -> Sender<TopologyLocation> {
        self.tx
    }
}
