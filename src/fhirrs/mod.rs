pub mod allergy_intolerance;
pub mod basic;
pub mod common;
pub mod condition;
pub mod dispatch;
pub mod encounter;
pub mod fields;
pub mod immunization;
pub mod location;
pub mod medication;
pub mod meta;
pub mod observation;
pub mod organization;
pub mod patient;
pub mod practitioner;
pub mod procedure;
pub mod unstructured;

#[cfg(test)]
pub mod testing {
    use serde_json::{json, Value};

    pub fn meta() -> Value {
        json!({"extension": []})
    }
}
