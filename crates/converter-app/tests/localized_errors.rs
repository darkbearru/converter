use converter_app::AppError;
use converter_core::model::ConverterError;
#[test]
fn параметры_сообщения_переживают_переход_между_слоями() {
    let error: ConverterError = serde_json::from_value(serde_json::json!({
        "code":"io_error", "message":"old fallback", "source_location":null,
        "message_key":"messages.test", "message_args":{"name":"<b>{name}</b>"}
    }))
    .expect("error");
    let value = serde_json::to_value(AppError::from(error)).expect("json");
    assert_eq!(value["message_key"], "messages.test");
    assert_eq!(value["message_args"]["name"], "<b>{name}</b>");
}
