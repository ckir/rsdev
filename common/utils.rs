use chrono::{DateTime, Utc};

pub fn current_datetime_rfc9557() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

pub fn print_type<T>(_: &T) {
    println!("{:?}", std::any::type_name::<T>());
}