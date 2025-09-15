#[cfg(test)]
mod tests {
    use super::super::gateway_configmaps::TemplateValues;
    use gtmpl::{Context, Template};
    use k8s_openapi::api::core::v1::ConfigMap;
    use serde_yaml;

    const TEMPLATE_CONTENT: &str =
        include_str!("../templates/gateway_configmap.kubernetes-helm-yaml");

    #[test]
    fn test_configmap_template_is_not_empty() {
        // Ensure the template file is not empty (this was the root cause of the bug)
        assert!(
            !TEMPLATE_CONTENT.trim().is_empty(),
            "ConfigMap template must not be empty"
        );

        // Ensure it contains expected ConfigMap structure
        assert!(TEMPLATE_CONTENT.contains("apiVersion: v1"));
        assert!(TEMPLATE_CONTENT.contains("kind: ConfigMap"));
        assert!(TEMPLATE_CONTENT.contains("metadata:"));
        assert!(TEMPLATE_CONTENT.contains("data:"));
    }

    #[test]
    fn test_configmap_template_renders_correctly() {
        // Create a template instance
        let mut template = Template::default();

        // Add required template functions (same as in sync_objects macro)
        use sprig::{
            defaults::default,
            strings::{indent, nindent},
        };
        template.add_func("default", default);
        template.add_func("indent", indent);
        template.add_func("nindent", nindent);

        // Parse the template
        template
            .parse(TEMPLATE_CONTENT)
            .expect("ConfigMap template should parse successfully");

        // Create test template values
        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway")
            .config_yaml("listeners:\n  - name: http\n    port: 8080")
            .build();

        // Render the template
        let context = Context::from(template_values);
        let rendered = template
            .render(&context)
            .expect("ConfigMap template should render successfully");

        // Parse the rendered YAML to ensure it's valid
        let configmap: ConfigMap = serde_yaml::from_str(&rendered)
            .expect("Rendered template should be valid ConfigMap YAML");

        // Verify the rendered ConfigMap has expected properties
        assert_eq!(
            configmap.metadata.name,
            Some("test-gateway-configuration".to_string())
        );
        assert!(configmap.data.is_some());

        let data = configmap.data.unwrap();
        assert!(data.contains_key("gateway.yaml"));

        let gateway_yaml = data.get("gateway.yaml").unwrap();
        assert!(gateway_yaml.contains("listeners:"));
        assert!(gateway_yaml.contains("- name: http"));
        assert!(gateway_yaml.contains("port: 8080"));
    }

    #[test]
    fn test_configmap_template_handles_special_characters() {
        let mut template = Template::default();

        // Add required template functions
        use sprig::{
            defaults::default,
            strings::{indent, nindent},
        };
        template.add_func("default", default);
        template.add_func("indent", indent);
        template.add_func("nindent", nindent);

        template
            .parse(TEMPLATE_CONTENT)
            .expect("ConfigMap template should parse successfully");

        // Test with config containing special characters
        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway-with-dashes")
            .config_yaml("config:\n  special: \"quotes and \\\"escapes\\\"\"\n  unicode: \"测试\"")
            .build();

        let context = Context::from(template_values);
        let rendered = template
            .render(&context)
            .expect("ConfigMap template should handle special characters");

        // Ensure the rendered YAML is still valid
        let configmap: ConfigMap = serde_yaml::from_str(&rendered)
            .expect("Rendered template with special characters should be valid");

        assert_eq!(
            configmap.metadata.name,
            Some("test-gateway-with-dashes-configuration".to_string())
        );
    }

    #[test]
    fn test_configmap_template_has_required_labels() {
        let mut template = Template::default();

        // Add required template functions
        use sprig::{
            defaults::default,
            strings::{indent, nindent},
        };
        template.add_func("default", default);
        template.add_func("indent", indent);
        template.add_func("nindent", nindent);

        template
            .parse(TEMPLATE_CONTENT)
            .expect("ConfigMap template should parse successfully");

        let template_values = TemplateValues::builder()
            .gateway_name("test-gateway")
            .config_yaml("listeners: []")
            .build();

        let context = Context::from(template_values);
        let rendered = template
            .render(&context)
            .expect("ConfigMap template should render successfully");

        let configmap: ConfigMap = serde_yaml::from_str(&rendered)
            .expect("Rendered template should be valid ConfigMap YAML");

        // Verify required labels are present
        let labels = configmap
            .metadata
            .labels
            .expect("ConfigMap should have labels");

        assert!(labels.contains_key("vale-gateway.whitefamily.in/configmap-role"));
        assert_eq!(
            labels.get("vale-gateway.whitefamily.in/configmap-role"),
            Some(&"gateway-configuration".to_string())
        );

        assert!(labels.contains_key("app.kubernetes.io/name"));
        assert_eq!(
            labels.get("app.kubernetes.io/name"),
            Some(&"test-gateway".to_string())
        );

        assert!(labels.contains_key("gateway.networking.k8s.io/gateway"));
        assert_eq!(
            labels.get("gateway.networking.k8s.io/gateway"),
            Some(&"test-gateway".to_string())
        );

        assert!(labels.contains_key("app"));
        assert_eq!(labels.get("app"), Some(&"test-gateway".to_string()));
    }
}
