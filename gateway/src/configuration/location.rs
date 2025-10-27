use tokio::spawn;
use typed_builder::TypedBuilder;
use vg_core::net::topology::TopologyLocation;
use vg_core::sync::handles::{Handle, handles};
use vg_core::sync::observable::{Observable, Subscription};

#[derive(TypedBuilder)]
pub struct CurrentLocationConfigurator {
    location: Observable<TopologyLocation>,
}

impl CurrentLocationConfigurator {
    pub fn new() -> Self {
        let location = Observable::default();
        Self::builder().location(location).build()
    }

    pub fn subscribe(&self) -> Subscription<TopologyLocation> {
        self.location.subscribe()
    }

    pub fn start(self) -> Handle {
        let (handle, mut stop_handle) = handles();

        spawn(async move {
            self.location
                .update(|_| TopologyLocation::builder().node(None).zone(None).build());
        });

        handle
    }
}
