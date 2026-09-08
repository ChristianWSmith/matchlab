//! Artifact and data formats: stable serialization formats
//! for research artifacts with versioning, inspectability, and
//! language interoperability.
/// Supported artifact formats.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactFormat {
    Json,
    Jsonl,
    Yaml,
}
/// Configuration for artifact export formats.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportConfig {
    pub experiment_format: ArtifactFormat,
    pub observation_format: ArtifactFormat,
    pub result_format: ArtifactFormat,
    pub checkpoint_format: ArtifactFormat,
}
impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            experiment_format: ArtifactFormat::Json,
            observation_format: ArtifactFormat::Jsonl,
            result_format: ArtifactFormat::Json,
            checkpoint_format: ArtifactFormat::Json,
        }
    }
}
/// A versioned wrapper for any serializable artifact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VersionedArtifact<T> {
    pub format_version: String,
    pub engine_version: String,
    pub content: T,
}
impl<T: serde::Serialize + serde::de::DeserializeOwned> VersionedArtifact<T> {
    pub fn new(content: T, engine_version: &str) -> Self {
        Self {
            format_version: "1.0.0".to_string(),
            engine_version: engine_version.to_string(),
            content,
        }
    }
    pub fn serialize_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
    pub fn deserialize_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
/// Validate that a parsed artifact has an expected format version.
pub fn validate_format_version(
    artifact: &VersionedArtifact<serde_json::Value>,
    expected: &str,
) -> Result<(), String> {
    if artifact.format_version != expected {
        return Err(format!(
            "format version mismatch: expected '{}', got '{}'",
            expected, artifact.format_version
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn artifact_format_serde_round_trip() {
        let formats = vec![
            ArtifactFormat::Json,
            ArtifactFormat::Jsonl,
            ArtifactFormat::Yaml,
        ];
        for format in &formats {
            let json = serde_json::to_string(format).unwrap();
            let back: ArtifactFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(*format, back);
        }
    }
    #[test]
    fn export_config_defaults() {
        let config = ExportConfig::default();
        assert_eq!(config.experiment_format, ArtifactFormat::Json);
        assert_eq!(config.observation_format, ArtifactFormat::Jsonl);
    }
    #[test]
    fn versioned_artifact_round_trip() {
        let data = serde_json::json!({"key": "value"});
        let artifact = VersionedArtifact::new(data.clone(), "0.8.0");
        let json = artifact.serialize_json().unwrap();
        let back: VersionedArtifact<serde_json::Value> =
            VersionedArtifact::deserialize_json(&json).unwrap();
        assert_eq!(back.format_version, "1.0.0");
        assert_eq!(back.engine_version, "0.8.0");
        assert_eq!(back.content, data);
    }
    #[test]
    fn validate_format_version_ok() {
        let artifact = VersionedArtifact::new(serde_json::json!({}), "0.8.0");
        assert!(validate_format_version(&artifact, "1.0.0").is_ok());
    }
    #[test]
    fn validate_format_version_mismatch() {
        let artifact = VersionedArtifact::new(serde_json::json!({}), "0.8.0");
        assert!(validate_format_version(&artifact, "2.0.0").is_err());
    }
}
