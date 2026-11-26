//! `Date`, in UTC: mapping scripts only measure intervals with it.

use std::rc::Rc;

use crate::value::*;

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
}

/// Days from 1970-01-01 to a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Year, month (0-11), day, hours, minutes, seconds, ms, weekday.
fn civil(t: f64) -> [f64; 8] {
    let ms = t.rem_euclid(1000.0);
    let secs = (t / 1000.0).floor() as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let wd = (days + 4).rem_euclid(7);
    [y as f64, (m - 1) as f64, d as f64, (rem / 3600) as f64, (rem / 60 % 60) as f64, (rem % 60) as f64, ms, wd as f64]
}

fn time(rt: &Rt, this: &Value) -> R<f64> {
    match this.as_obj().and_then(|o| o.props.borrow().get("\0time").cloned()) {
        Some(Prop::Method(Value::Num(t))) => Ok(t),
        _ => rt.type_error("not a Date"),
    }
}

fn method(rt: &Rt, o: &Rc<Obj>, name: &str, f: impl Fn(&Rt, &Value, &[Value]) -> R<Value> + 'static) {
    let v = rt.native(name, f);
    o.props.borrow_mut().set(&name.into(), Prop::Method(v));
}

pub fn install(rt: &Rt) {
    let proto = Obj::new(Some(rt.object_proto.clone()), Kind::Plain);
    let ctor = rt.native("Date", |rt, this, a| {
        let t = match a {
            [] => now_ms(),
            [v] => rt.to_num(v)?,
            _ => {
                let n: Vec<f64> = a.iter().map(|v| rt.to_num(v)).collect::<R<_>>()?;
                let g = |i: usize, d: f64| n.get(i).copied().unwrap_or(d);
                let y = g(0, 1970.0) as i64 + g(1, 0.0).div_euclid(12.0) as i64;
                let m = g(1, 0.0).rem_euclid(12.0) as i64 + 1;
                days_from_civil(y, m, 1) as f64 * 86400000.0
                    + (g(2, 1.0) - 1.0) * 86400000.0
                    + g(3, 0.0) * 3600000.0
                    + g(4, 0.0) * 60000.0
                    + g(5, 0.0) * 1000.0
                    + g(6, 0.0)
            }
        };
        match this {
            Value::Obj(o) if !Rc::ptr_eq(o, &rt.global) => {
                o.props.borrow_mut().set(&"\0time".into(), Prop::Method(Value::Num(t)));
                Ok(this.clone())
            }
            _ => Ok(Value::Str(crate::number_to_string(now_ms()).into())),
        }
    });
    let co = ctor.as_obj().unwrap();
    co.props.borrow_mut().set(&rt.prototype, Prop::Method(Value::Obj(proto.clone())));
    proto.props.borrow_mut().set(&rt.constructor, Prop::Method(ctor.clone()));
    method(rt, co, "now", |_, _, _| Ok(Value::Num(now_ms())));
    method(rt, &proto, "getTime", |rt, this, _| time(rt, this).map(Value::Num));
    method(rt, &proto, "valueOf", |rt, this, _| time(rt, this).map(Value::Num));
    for (i, names) in [
        ["getFullYear", "getUTCFullYear"],
        ["getMonth", "getUTCMonth"],
        ["getDate", "getUTCDate"],
        ["getHours", "getUTCHours"],
        ["getMinutes", "getUTCMinutes"],
        ["getSeconds", "getUTCSeconds"],
        ["getMilliseconds", "getUTCMilliseconds"],
        ["getDay", "getUTCDay"],
    ]
    .into_iter()
    .enumerate()
    {
        for n in names {
            method(rt, &proto, n, move |rt, this, _| {
                let t = time(rt, this)?;
                Ok(Value::Num(if t.is_nan() { f64::NAN } else { civil(t)[i] }))
            });
        }
    }
    method(rt, &proto, "getTimezoneOffset", |_, _, _| Ok(Value::Num(0.0)));
    method(rt, &proto, "toISOString", |rt, this, _| {
        let c = civil(time(rt, this)?);
        Ok(Value::Str(
            format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", c[0], c[1] + 1.0, c[2], c[3], c[4], c[5], c[6])
                .into(),
        ))
    });
    method(rt, &proto, "toString", |rt, this, _| {
        let iso = rt.get(this, "toISOString")?;
        rt.call(&iso, this.clone(), &[])
    });
    rt.global.props.borrow_mut().set(&"Date".into(), Prop::Method(ctor));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip() {
        let t = days_from_civil(2026, 11, 16) as f64 * 86400000.0 + 3723004.0;
        assert_eq!(civil(t), [2026.0, 10.0, 16.0, 1.0, 2.0, 3.0, 4.0, 1.0]);
        assert_eq!(civil(0.0)[..3], [1970.0, 0.0, 1.0]);
    }
}
