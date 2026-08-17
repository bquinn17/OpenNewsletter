//! Update-expression building blocks.

use aws_sdk_dynamodb::operation::update_item::builders::UpdateItemFluentBuilder;
use aws_sdk_dynamodb::types::AttributeValue;

/// Bind each `(attribute, value)` pair to numbered `#f{n}` / `:f{n}` placeholders and
/// return the `SET` assignment fragments.
///
/// Aliasing every attribute name — not just the ones that need it today — keeps
/// DynamoDB's reserved-word list (`name`, `timezone`, `status`, `role`, …) from
/// being something each new patch field has to be checked against.
pub fn set_fields(
    mut update: UpdateItemFluentBuilder,
    fields: Vec<(&str, AttributeValue)>,
) -> (UpdateItemFluentBuilder, Vec<String>) {
    let mut assignments = Vec::with_capacity(fields.len());
    for (index, (attribute, value)) in fields.into_iter().enumerate() {
        let name_placeholder = format!("#f{index}");
        let value_placeholder = format!(":f{index}");
        assignments.push(format!("{name_placeholder} = {value_placeholder}"));
        update = update
            .expression_attribute_names(name_placeholder, attribute)
            .expression_attribute_values(value_placeholder, value);
    }
    (update, assignments)
}
