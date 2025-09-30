use schemars::{Schema, SchemaGenerator, json_schema};

pub fn cidr_array_schema(_: &mut SchemaGenerator) -> Schema {
    // Create schema for a single CIDR
    let item_schema = json_schema!({
        "type": "string",
        "format": "cidr",
    });

    // Create schema for array of CIDRs
    json_schema!({
        "type": "array",
        "items": item_schema,
        "uniqueItems": true,
    })
}
