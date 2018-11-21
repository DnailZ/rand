use rand_core::{Error, ErrorKind};
use super::OsRngImpl;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    pub type Function;
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Function;
    #[wasm_bindgen(method)]
    pub fn call(this: &Function, self_: &JsValue) -> JsValue;

    pub type This;
    #[wasm_bindgen(method, getter, structural, js_name = self)]
    pub fn self_(me: &This) -> JsValue;
    #[wasm_bindgen(method, getter, structural)]
    pub fn crypto(me: &This) -> JsValue;

    #[derive(Clone, Debug)]
    pub type BrowserCrypto;

    // TODO: these `structural` annotations here ideally wouldn't be here to
    // avoid a JS shim, but for now with feature detection they're
    // unavoidable.
    #[wasm_bindgen(method, js_name = getRandomValues, structural, getter)]
    pub fn get_random_values_fn(me: &BrowserCrypto) -> JsValue;
    #[wasm_bindgen(method, js_name = getRandomValues, structural)]
    pub fn get_random_values(me: &BrowserCrypto, buf: &mut [u8]);

    #[wasm_bindgen(js_name = require)]
    pub fn node_require(s: &str) -> NodeCrypto;

    #[derive(Clone, Debug)]
    pub type NodeCrypto;

    #[wasm_bindgen(method, js_name = randomFillSync, structural)]
    pub fn random_fill_sync(me: &NodeCrypto, buf: &mut [u8]);
}

#[derive(Clone, Debug)]
pub enum OsRng {
    Node(NodeCrypto),
    Browser(BrowserCrypto),
}

impl OsRngImpl for OsRng {
    fn new() -> Result<OsRng, Error> {
        // First up we need to detect if we're running in node.js or a
        // browser. To do this we get ahold of the `this` object (in a bit
        // of a roundabout fashion).
        //
        // Once we have `this` we look at its `self` property, which is
        // only defined on the web (either a main window or web worker).
        let this = Function::new("return this").call(&JsValue::undefined());
        assert!(this != JsValue::undefined());
        let this = This::from(this);
        let is_browser = this.self_() != JsValue::undefined();

        if !is_browser {
            return Ok(OsRng::Node(node_require("crypto")))
        }

        // If `self` is defined then we're in a browser somehow (main window
        // or web worker). Here we want to try to use
        // `crypto.getRandomValues`, but if `crypto` isn't defined we assume
        // we're in an older web browser and the OS RNG isn't available.
        let crypto = this.crypto();
        if crypto.is_undefined() {
            let msg = "self.crypto is undefined";
            return Err(Error::new(ErrorKind::Unavailable, msg))
        }

        // Test if `crypto.getRandomValues` is undefined as well
        let crypto: BrowserCrypto = crypto.into();
        if crypto.get_random_values_fn().is_undefined() {
            let msg = "crypto.getRandomValues is undefined";
            return Err(Error::new(ErrorKind::Unavailable, msg))
        }

        // Ok! `self.crypto.getRandomValues` is a defined value, so let's
        // assume we can do browser crypto.
        Ok(OsRng::Browser(crypto))
    }

    fn fill_chunk(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        match *self {
            OsRng::Node(ref n) => n.random_fill_sync(dest),
            OsRng::Browser(ref n) => n.get_random_values(dest),
        }
        Ok(())
    }

    fn max_chunk_size(&self) -> usize {
        match *self {
            OsRng::Node(_) => usize::max_value(),
            OsRng::Browser(_) => {
                // see https://developer.mozilla.org/en-US/docs/Web/API/Crypto/getRandomValues
                //
                // where it says:
                //
                // > A QuotaExceededError DOMException is thrown if the
                // > requested length is greater than 65536 bytes.
                65536
            }
        }
    }

    fn method_str(&self) -> &'static str {
        match *self {
            OsRng::Node(_) => "crypto.randomFillSync",
            OsRng::Browser(_) => "crypto.getRandomValues",
        }
    }
}