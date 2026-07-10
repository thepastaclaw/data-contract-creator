#[cfg(test)]
mod tests {
    use super::super::json_generator::JsonGenerator;
    use super::super::json_parser::JsonParser;
    use super::super::validation::ValidationService;
    use crate::types::{DataType, DocumentType, Property};

    #[test]
    fn test_empty_contract_validation() {
        let empty_json = "{}";
        let result = ValidationService::validate_schema(empty_json);

        assert!(result.is_ok());
        let errors = result.unwrap();
        assert!(
            !errors.is_empty(),
            "Empty contract should have validation errors"
        );
        println!("Empty contract errors: {:?}", errors);
    }

    #[test]
    fn test_minimal_valid_contract() {
        let doc_type = DocumentType {
            name: "testDoc".to_string(),
            properties: vec![Property {
                name: "name".to_string(),
                data_type: DataType::String,
                required: true,
                max_length: Some(50),
                ..Default::default()
            }],
            ..Default::default()
        };

        let json = JsonGenerator::generate_contract(&[doc_type]);
        let json_str = serde_json::to_string(&json).unwrap();

        println!("Generated JSON: {}", json_str);

        let result = ValidationService::validate_schema(&json_str);
        assert!(result.is_ok());

        let errors = result.unwrap();
        println!("Validation errors: {:?}", errors);

        // A valid contract should have no errors
        assert!(
            errors.is_empty(),
            "Valid contract should not have validation errors"
        );
    }

    #[test]
    fn test_default_document_type() {
        let default_doc = DocumentType::default();
        let json = JsonGenerator::generate_contract(&[default_doc]);
        let json_str = serde_json::to_string(&json).unwrap();

        println!("Default document JSON: {}", json_str);

        let result = ValidationService::validate_schema(&json_str);
        assert!(result.is_ok());

        let errors = result.unwrap();
        println!("Default document validation errors: {:?}", errors);
    }

    #[test]
    fn test_array_byte_array_validation() {
        let mut doc_type = DocumentType::default();
        doc_type.name = "testDoc".to_string();
        doc_type.properties.push(Property {
            name: "data".to_string(),
            data_type: DataType::Array,
            required: true,
            ..Default::default()
        });

        let json = JsonGenerator::generate_contract(&[doc_type]);
        let json_str = serde_json::to_string(&json).unwrap();

        println!("Array property JSON: {}", json_str);

        let errors = ValidationService::validate_byte_arrays(&json_str);
        println!("Byte array validation errors: {:?}", errors);

        // Should not have errors since JsonGenerator automatically adds byteArray: true
        assert!(
            errors.is_empty(),
            "Array with byteArray: true should not have validation errors"
        );
    }

    #[test]
    fn test_app_initial_state_behavior() {
        // Simulate app initial state - single default document type
        let default_docs = vec![DocumentType::default()];
        let json = JsonGenerator::generate_contract(&default_docs);
        let json_str = serde_json::to_string(&json).unwrap();

        println!("App initial state JSON: {}", json_str);

        // This should be empty because default doc has empty name
        assert_eq!(json_str, "{}", "Initial state should produce empty JSON");

        // But validation should still work when explicitly called
        let result = ValidationService::validate_schema(&json_str);
        assert!(result.is_ok());

        let errors = result.unwrap();
        println!("Initial state validation errors: {:?}", errors);

        // Empty JSON should fail validation (user needs to add content)
        assert!(!errors.is_empty(), "Empty contract should fail validation");
    }

    #[test]
    fn ai_generation_pipeline_parses_regenerates_and_validates() {
        // Mirrors the data path AppMsg::AiGenerationComplete -> ValidateContract
        // drives: parse the model's schema into document types, regenerate the
        // contract JSON from them, then run DPP validation. The Yew message
        // dispatch itself needs a browser/wasm harness and is not exercised here;
        // this locks in that the underlying pipeline runs end-to-end.
        let ai_schema = r#"{"note":{"type":"object","description":"A note","$comment":"internal","properties":{"title":{"position":0,"type":"string","description":"Title","maxLength":63}},"indices":[{"name":"title","properties":[{"title":"asc"}]}],"required":["title"],"additionalProperties":false}}"#;

        let doc_types = JsonParser::parse_contract(ai_schema).expect("AI schema should parse");
        let regenerated = JsonGenerator::generate_contract(&doc_types);
        let json_str = serde_json::to_string(&regenerated).unwrap();

        // The parse -> generate round-trip preserved the document type and its
        // $comment (the field the prompt now instructs the model to emit).
        assert!(json_str.contains("\"note\""), "doc type lost: {}", json_str);
        assert!(
            json_str.contains("\"$comment\""),
            "comment lost: {}",
            json_str
        );

        // Validation actually executes on the regenerated contract (issue #31).
        let result = ValidationService::validate_schema(&json_str);
        assert!(result.is_ok(), "validation should execute: {:?}", result);
    }

    // Minimal valid contested-index document schema, modeled on the pinned DPP's
    // DPNS `domain` document type: an immutable (`documentsMutable: false`)
    // document type whose single unique index carries a `contested` object with
    // `resolution: 0` (masternode vote) and a `fieldMatches` regex. This is the
    // shape the prompt now instructs the model to produce.
    const CONTESTED_SCHEMA: &str = r#"{
        "card": {
            "type": "object",
            "documentsMutable": false,
            "properties": {
                "name": {"position": 0, "type": "string", "maxLength": 63, "description": "Card name"}
            },
            "indices": [{
                "name": "byName",
                "properties": [{"name": "asc"}],
                "unique": true,
                "contested": {
                    "fieldMatches": [{"field": "name", "regexPattern": "^[a-zA-Z0-9]{3,19}$"}],
                    "resolution": 0,
                    "description": "Contested name resolved by masternode vote"
                }
            }],
            "required": ["name"],
            "additionalProperties": false
        }
    }"#;

    #[test]
    fn contested_document_round_trips_and_validates_without_mutable_error() {
        // Parse -> regenerate: documentsMutable and the contested object must
        // survive the trip through the internal model.
        let doc_types =
            JsonParser::parse_contract(CONTESTED_SCHEMA).expect("contested schema parses");
        assert_eq!(doc_types[0].documents_mutable, Some(false));
        assert!(doc_types[0].indices[0].contested.is_some());

        let regenerated = JsonGenerator::generate_contract(&doc_types);
        let json_str = serde_json::to_string(&regenerated).unwrap();
        assert_eq!(
            regenerated["card"]["documentsMutable"],
            serde_json::json!(false)
        );
        assert!(regenerated["card"]["indices"][0]["contested"].is_object());

        // DPP must accept the regenerated contract. In particular it must NOT
        // raise ContestedUniqueIndexOnMutableDocumentTypeError, which is exactly
        // what happens if documentsMutable is lost.
        let errors = ValidationService::validate_schema(&json_str)
            .expect("validation should execute for contested contract");
        let joined = errors
            .iter()
            .map(|e| e.display_message())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(
            !joined.contains("mutable") && !joined.to_lowercase().contains("contested"),
            "contested contract wrongly rejected: {}",
            joined
        );
        assert!(
            errors.is_empty(),
            "contested contract should validate cleanly, got: {}",
            joined
        );
    }

    #[test]
    fn contested_document_without_documents_mutable_is_rejected_by_dpp() {
        // Guard test: the same schema WITHOUT `documentsMutable: false` must be
        // rejected, proving the flag is load-bearing (documents default to
        // mutable) and that preserving it is what keeps the contract valid.
        let without_flag = CONTESTED_SCHEMA.replace("\"documentsMutable\": false,", "");
        let errors =
            ValidationService::validate_schema(&without_flag).expect("validation should execute");
        // DPP rejects it with ContestedUniqueIndexOnMutableDocumentTypeError
        // ("...has a contested unique index ... but is set as mutable").
        let joined = errors
            .iter()
            .map(|e| e.display_message())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(
            joined.contains("mutable"),
            "expected the contested-on-mutable rejection, got: {}",
            joined
        );
    }
}
