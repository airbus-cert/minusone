use crate::js::JavaScript;
use crate::js::JavaScript::{NaN, Object, Raw};
use crate::js::Value::Num;
use crate::scope::Scope;
use std::collections::HashMap;

fn number_obj() -> JavaScript {
    let mut number = HashMap::new();
    number.insert("MAX_VALUE".to_string(), Raw(Num(f64::MAX)));
    number.insert("MIN_VALUE".to_string(), Raw(Num(f64::MIN_POSITIVE)));
    number.insert("MAX_SAFE_INTEGER".to_string(), Raw(Num(9007199254740991.0)));
    number.insert(
        "MIN_SAFE_INTEGER".to_string(),
        Raw(Num(-9007199254740991.0)),
    );
    number.insert("POSITIVE_INFINITY".to_string(), Raw(Num(f64::INFINITY)));
    number.insert("NEGATIVE_INFINITY".to_string(), Raw(Num(f64::NEG_INFINITY)));
    number.insert("NaN".to_string(), NaN);
    number.insert("EPSILON".to_string(), Raw(Num(f64::EPSILON)));
    Object(number)
}

fn js_global_objects() -> HashMap<String, JavaScript> {
    let mut globals = HashMap::new();
    globals.insert("Number".to_string(), number_obj());
    globals
}

pub fn inject_js_globals(scope: &mut Scope<JavaScript>, ongoing_transaction: bool) {
    for (name, value) in js_global_objects() {
        scope.assign(&name, value, ongoing_transaction);
    }
}
