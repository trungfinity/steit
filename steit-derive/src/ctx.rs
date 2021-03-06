use std::{cell::RefCell, fmt, thread};

use quote::ToTokens;

pub struct Context {
    errors: RefCell<Option<Vec<syn::Error>>>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            errors: RefCell::new(Some(Vec::new())),
        }
    }

    pub fn error(&self, tokens: impl ToTokens, message: impl fmt::Display) {
        self.syn_error(syn::Error::new_spanned(tokens, message));
    }

    pub fn syn_error(&self, error: syn::Error) {
        self.errors.borrow_mut().as_mut().unwrap().push(error);
    }

    pub fn check(self) -> Result<(), Vec<syn::Error>> {
        let errors = self.errors.borrow_mut().take().unwrap();

        match errors.len() {
            0 => Ok(()),
            _ => Err(errors),
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if !thread::panicking() && self.errors.borrow().is_some() {
            panic!("forgot to check for errors?");
        }
    }
}
