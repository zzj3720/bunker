///|
extern "C" fn link_start() = "bunker_link_start"

///|
#borrow(json)
extern "C" fn link_call(id : Int64, json : Bytes) = "bunker_link_call"

///|
#borrow(json, data)
extern "C" fn link_call_data(id : Int64, json : Bytes, data : Bytes) = "bunker_link_call_data"

///|
/// The core told where its answers go (the "core" channel); once, as the app starts.
pub fn start() -> Unit {
  link_start()
}

///|
/// Request `json` sent as `id`; its answer comes on the "core" channel.
pub fn call(id : Int64, json : String) -> Unit {
  link_call(id, @utf8.encode(json))
}

///|
/// As `call`, with bytes beside it.
pub fn call_data(id : Int64, json : String, data : Bytes) -> Unit {
  link_call_data(id, @utf8.encode(json), data)
}
