use crate::js::JavaScript;
use crate::js::JavaScript::{NaN, Object, Raw};
use crate::js::Value::Num;
use crate::scope::Scope;
use std::collections::HashMap;
use std::f64::consts::*;

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

fn math_obj() -> JavaScript {
    let mut number = HashMap::new();
    number.insert("E".to_string(), Raw(Num(E)));
    number.insert("LN10".to_string(), Raw(Num(LN_10)));
    number.insert("LN2".to_string(), Raw(Num(LN_2)));
    number.insert("LOG10E".to_string(), Raw(Num(LOG10_E)));
    number.insert("LOG2E".to_string(), Raw(Num(LOG2_E)));
    number.insert("PI".to_string(), Raw(Num(PI)));
    number.insert("SQRT2".to_string(), Raw(Num(SQRT_2)));
    number.insert("SQRT1_2".to_string(), Raw(Num(FRAC_1_SQRT_2)));
    Object(number)
}

fn js_global_objects() -> HashMap<String, JavaScript> {
    let mut globals = HashMap::new();
    globals.insert("Number".to_string(), number_obj());
    globals.insert("Math".to_string(), math_obj());
    globals
}

pub fn inject_js_globals(scope: &mut Scope<JavaScript>, ongoing_transaction: bool) {
    for (name, value) in js_global_objects() {
        scope.assign(&name, value, ongoing_transaction);
    }
}
