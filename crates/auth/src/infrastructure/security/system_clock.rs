use crate::application::Clock;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        (js_sys::Date::now() / 1000.0) as i64
    }
}
