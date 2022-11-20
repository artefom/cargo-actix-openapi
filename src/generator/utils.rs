use openapiv3::{Components, ReferenceOr};

/// Dereference openapi object
pub fn deref<'a, T>(_components: &Option<Components>, obj: &'a ReferenceOr<T>) -> &'a T {
    let _obj_ref = match obj {
        ReferenceOr::Reference { reference } => reference,
        ReferenceOr::Item(value) => return value,
    };
    todo!()
}
