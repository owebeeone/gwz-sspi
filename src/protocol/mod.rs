mod adapters;
mod cbor;
mod framing;
mod generated;
mod profile;

#[cfg(test)]
mod tests {
    include!("tests.rs");
}

#[cfg(test)]
mod test_vectors {
    include!("test_vectors.rs");
}

#[cfg(test)]
mod bounds_tests {
    include!("bounds_tests.rs");
}

#[cfg(test)]
mod framing_tests {
    include!("framing_tests.rs");
}

#[cfg(test)]
mod identity_adapter_tests {
    include!("identity_adapter_tests.rs");
}
