use std::{any::Any, borrow::Cow};

pub fn panic_message<'a>(payload: &'a (dyn Any + Send + 'static)) -> Cow<'a, str> {
    payload
        .downcast_ref::<&'static str>()
        .map(|s| Cow::Borrowed(*s))
        .or_else(|| {
            payload
                .downcast_ref::<String>()
                .map(|s| Cow::Borrowed(s.as_str()))
        })
        .unwrap_or_else(|| Cow::Owned(format!("payload of type {:?}", payload.type_id())))
}
