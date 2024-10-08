import { invoke } from "@tauri-apps/api/core";

function Create() {
  function stop() {
    invoke("stop_server");
  }

  function teste() {
    const password = "321";

    invoke("start_server", { password });
  }

  function status() {
    invoke("get_server_status");
  }

  function broadcast_message() {
    invoke("broadcast_message_command", { message: "funcionouuuuu" });
  }

  return (
    <div>
      <button onClick={teste}>start</button>
      <button onClick={status}>status</button>
      <button onClick={stop}>stop</button>
      <button onClick={broadcast_message}>broadcast message</button>
    </div>
  );
}

export default Create;
