use kube::CustomResourceExt;
use schemars::schema_for;
use std::fs::create_dir_all;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use vg_kube::api::v1::parameters::{GatewayClassParameters, GatewayParameters};
use vg_kube::api::v1::parameters::listeners::http::filters::{AccessControlFilter, ClientAddressFilter, ErrorResponseFilter, StaticResponseFilter};

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(manifest_dir.as_str());
    
    let out_dir = manifest_dir
        .join("..")
        .join("helm")
        .join("crds")
        .join("generated");

    let out_dir = out_dir.as_path();
    write_kube_crds(out_dir);
}

fn write_kube_crds(out_dir: &Path) {
    create_dir_all(out_dir).unwrap();
    let dest_path = out_dir.join("crds.yaml");
    let file = File::create(dest_path).unwrap();

    [
        GatewayClassParameters::crd(),
        GatewayParameters::crd(),
        AccessControlFilter::crd(),
        ErrorResponseFilter::crd(),
        ClientAddressFilter::crd(),
        StaticResponseFilter::crd(),
    ]
    .iter()
    .fold(file, |mut output, crd| {
        writeln!(output, "---").unwrap();
        writeln!(output, "{}", serde_yaml::to_string(crd).unwrap().as_str()).unwrap();
        output
    });
}
