//! The schema-profile pass: the shared Structured-Output schema object,
//! rendered into the narrower JSON-Schema dialect this provider's structured
//! outputs accept.
//!
//! **This is a subtraction, never a rewrite.** `transync::llm::prompt` owns
//! the schema objects (tier (b), contracts.md §0) and this module must not
//! reshape them — it removes a fixed, named set of keywords the provider's
//! dialect does not accept and leaves every other byte alone. The provider
//! demands `additionalProperties: false` and `required`, which the shared
//! objects already carry, so nothing has to be *added*.
//!
//! Two keywords are actually reachable today: `minItems`/`maxItems`, stamped
//! on the translation schema's `units` array by
//! `prompt::schema_object_for(Some(n))` and on the extraction schema's
//! `terms` array by `prompt::extraction_schema_object(max_terms)`. The rest
//! of [`UNSUPPORTED_KEYWORDS`] is the same dialect class, listed so a future
//! change to a shared schema is handled rather than silently sent.
//!
//! Dropping the array cap costs nothing downstream: the translation unit
//! count is re-checked by core's ID-set validator, and `extract_glossary`
//! truncates the parsed list to `req.max_terms` post-parse, so the
//! `GlossaryExtractionRequest` contract holds regardless of what the provider
//! did with the keyword.
//!
//! TRACE: DCR-0029
//! TRACE: contracts.md §8

use serde_json::Value;

/// Keywords this provider's structured-output dialect does not accept:
/// numerical constraints, string constraints, and array/object count
/// constraints.
///
/// Deliberately a fixed list rather than an allowlist of *accepted*
/// keywords. An allowlist would silently drop anything the shared schema
/// grows next — including a keyword the provider does accept — which is the
/// failure direction that costs correctness rather than a request.
const UNSUPPORTED_KEYWORDS: &[&str] = &[
    // array counts
    "minItems",
    "maxItems",
    "uniqueItems",
    "minContains",
    "maxContains",
    // numeric bounds
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    // string bounds
    "minLength",
    "maxLength",
    "pattern",
    // object counts
    "minProperties",
    "maxProperties",
];

/// Keys whose *value* is a map of arbitrary names to sub-schemas, not a map
/// of schema keywords.
///
/// The distinction is load-bearing: a document with a property literally
/// named `maxItems` must keep it, because at that position the string is a
/// field name in the translated payload's shape and not a constraint on it.
/// Recursing blindly would delete it.
///
/// The list is the standard JSON-Schema set of name→schema maps, not just the
/// ones a shared schema reaches today: `dependentSchemas` joined on that rule
/// (R0009-0086), where the map's keys are *property names* that trigger their
/// sub-schema. No shared object in `transync::llm::prompt` emits one yet, so
/// this is the traversal being right rather than a live defect being fixed —
/// which is the same reason `patternProperties` and `definitions` are here.
const NAMED_SUBSCHEMA_MAPS: &[&str] = &[
    "properties",
    "patternProperties",
    "dependentSchemas",
    "$defs",
    "definitions",
];

/// Render one shared schema object into the provider's dialect.
///
/// Deterministic and total: the same input always yields the same output,
/// and no keyword outside [`UNSUPPORTED_KEYWORDS`] is touched.
pub(super) fn to_provider_dialect(mut schema: Value) -> Value {
    strip_schema(&mut schema);
    schema
}

/// Strip in place, treating `value` as a schema node.
fn strip_schema(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for keyword in UNSUPPORTED_KEYWORDS {
                map.remove(*keyword);
            }
            for (key, child) in map.iter_mut() {
                if NAMED_SUBSCHEMA_MAPS.contains(&key.as_str()) {
                    strip_named_subschema_map(child);
                } else {
                    strip_schema(child);
                }
            }
        }
        // `anyOf` / `allOf` / `oneOf` / `prefixItems` are arrays of schemas;
        // `required` and `enum` are arrays of scalars, which recursion leaves
        // untouched because a scalar has nothing to strip.
        Value::Array(items) => items.iter_mut().for_each(strip_schema),
        _ => {}
    }
}

/// Strip the *values* of a name→schema map without treating its keys as
/// keywords.
fn strip_named_subschema_map(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (_name, child) in map.iter_mut() {
                strip_schema(child);
            }
        }
        // Not a map — leave it for the provider to reject rather than
        // guessing at a shape the shared schema does not produce.
        other => strip_schema(other),
    }
}

/// Every keyword this pass removes, for the assertions that pin it.
#[cfg(test)]
pub(super) fn unsupported_keywords() -> &'static [&'static str] {
    UNSUPPORTED_KEYWORDS
}

/// Collect every object key appearing anywhere in `value`, so a test can
/// assert absence over the whole tree rather than at one remembered path.
#[cfg(test)]
pub(super) fn all_keys(value: &Value) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    out.push(k.clone());
                    walk(v, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(value, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use transync::llm::prompt;

    fn map_of(value: &Value) -> &serde_json::Map<String, Value> {
        value.as_object().expect("schema node is an object")
    }

    /// The translation schema loses its array count constraints and keeps
    /// everything else — including the two keywords the provider *demands*.
    #[test]
    fn the_translation_schema_loses_only_its_array_counts() {
        let shared = prompt::schema_object_for(Some(3));
        assert_eq!(
            shared["properties"]["units"]["maxItems"], 3,
            "precondition: the shared object stamps the cap this pass removes"
        );
        assert_eq!(shared["properties"]["units"]["minItems"], 3);

        let profiled = to_provider_dialect(shared.clone());
        let keys = all_keys(&profiled);
        for keyword in unsupported_keywords() {
            assert!(
                !keys.iter().any(|k| k == keyword),
                "{keyword} must not survive anywhere in the profiled schema"
            );
        }

        // What the provider demands is still there, at both levels.
        assert_eq!(profiled["additionalProperties"], false);
        assert!(profiled["required"].is_array());
        let item = &profiled["properties"]["units"]["items"];
        assert_eq!(item["additionalProperties"], false);
        assert_eq!(
            item["required"],
            serde_json::json!(["unit_id", "output_kind", "translated_payload", "warnings"])
        );
        // …and the parts that describe the answer's *shape* are untouched.
        assert_eq!(
            item["properties"]["output_kind"]["enum"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            map_of(&profiled["properties"]).keys().collect::<Vec<_>>(),
            vec!["detected_source_language", "units"]
        );
    }

    /// The extraction schema's `maxItems` — the one DCR-0029 names — goes the
    /// same way, and the term object survives intact.
    #[test]
    fn the_extraction_schema_loses_its_term_cap() {
        let shared = prompt::extraction_schema_object(9);
        assert_eq!(shared["properties"]["terms"]["maxItems"], 9);

        let profiled = to_provider_dialect(shared);
        assert!(
            profiled["properties"]["terms"].get("maxItems").is_none(),
            "the cap must not ride out: {profiled}"
        );
        assert_eq!(profiled["properties"]["terms"]["type"], "array");
        let item = &profiled["properties"]["terms"]["items"];
        assert_eq!(item["additionalProperties"], false);
        assert_eq!(
            item["required"],
            serde_json::json!(["source", "target", "note"])
        );
    }

    /// A shared schema with nothing to strip must come back byte-identical.
    /// Without this the pass could be quietly rewriting the object and the
    /// two tests above would still pass.
    #[test]
    fn a_schema_with_no_unsupported_keyword_is_returned_unchanged() {
        let shared = prompt::schema_object_for(None);
        assert_eq!(
            to_provider_dialect(shared.clone()),
            shared,
            "the pass subtracts; it must never reshape"
        );
    }

    /// The distinction that makes the pass safe: at a `properties` position
    /// the map's keys are *field names in the model's answer*, so a document
    /// whose schema declares a field called `maxItems` keeps it — while the
    /// constraint of the same spelling, one level up, is removed.
    #[test]
    fn a_property_named_like_a_keyword_is_not_a_keyword() {
        let schema = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["maxItems"],
            "maxItems": 4,
            "properties": {
                "maxItems": { "type": "string", "maxLength": 12 },
                "$defs": { "type": "string" }
            }
        });
        let profiled = to_provider_dialect(schema);
        assert!(
            profiled.get("maxItems").is_none(),
            "the constraint must go: {profiled}"
        );
        assert_eq!(
            profiled["properties"]["maxItems"]["type"], "string",
            "the field of the same name must stay: {profiled}"
        );
        assert!(
            profiled["properties"]["maxItems"]
                .get("maxLength")
                .is_none(),
            "…but constraints *inside* that field's schema still go: {profiled}"
        );
        assert_eq!(
            profiled["properties"]["$defs"]["type"], "string",
            "a field named like a subschema map is still just a field: {profiled}"
        );
        assert_eq!(profiled["required"], serde_json::json!(["maxItems"]));
    }

    /// `dependentSchemas` is a name→schema map like `properties`, so its keys
    /// are property names too: a field called `pattern` that triggers a
    /// sub-schema must survive, while the constraints *inside* that sub-schema
    /// still go (R0009-0086). Latent today — no shared schema emits the
    /// keyword — so this test is what makes the traversal right in advance
    /// rather than after a shared schema grows one.
    #[test]
    fn a_dependent_subschema_map_keeps_its_property_names() {
        let schema = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "dependentSchemas": {
                "pattern": {
                    "type": "object",
                    "properties": { "flags": { "type": "string", "maxLength": 4 } },
                    "minProperties": 1
                },
                "maxItems": { "type": "object", "required": ["units"] }
            }
        });
        let profiled = to_provider_dialect(schema);
        let mut dependents: Vec<&str> = map_of(&profiled["dependentSchemas"])
            .keys()
            .map(String::as_str)
            .collect();
        dependents.sort_unstable();
        assert_eq!(
            dependents,
            vec!["maxItems", "pattern"],
            "a dependent's name is a property name, not a keyword: {profiled}"
        );
        assert!(
            profiled["dependentSchemas"]["pattern"]
                .get("minProperties")
                .is_none(),
            "a constraint inside the dependent schema still goes: {profiled}"
        );
        assert!(
            profiled["dependentSchemas"]["pattern"]["properties"]["flags"]
                .get("maxLength")
                .is_none(),
            "…and the pass reaches the whole way down: {profiled}"
        );
        assert_eq!(
            profiled["dependentSchemas"]["maxItems"]["required"],
            serde_json::json!(["units"])
        );
    }

    /// Composition keywords carry arrays of schemas, and the pass has to
    /// reach through them — a constraint hidden in an `anyOf` branch is as
    /// rejectable as one at the root.
    #[test]
    fn constraints_inside_composition_branches_are_reached() {
        let schema = serde_json::json!({
            "anyOf": [
                { "type": "array", "items": { "type": "string" }, "minItems": 1 },
                { "type": "string", "pattern": "^x" }
            ],
            "$defs": {
                "Cap": { "type": "integer", "minimum": 0, "maximum": 9 }
            }
        });
        let profiled = to_provider_dialect(schema);
        for keyword in unsupported_keywords() {
            assert!(
                !all_keys(&profiled).iter().any(|k| k == keyword),
                "{keyword} survived a nested position: {profiled}"
            );
        }
        assert_eq!(profiled["anyOf"][0]["items"]["type"], "string");
        assert_eq!(profiled["$defs"]["Cap"]["type"], "integer");
    }

    /// Deterministic: the pass is pure, so two runs over the same input agree
    /// and a second pass over its own output is a no-op.
    #[test]
    fn the_pass_is_deterministic_and_idempotent() {
        let shared = prompt::schema_object_for(Some(7));
        let once = to_provider_dialect(shared.clone());
        assert_eq!(once, to_provider_dialect(shared));
        assert_eq!(once, to_provider_dialect(once.clone()));
    }
}
