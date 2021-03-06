extern crate tungstenite;
use pf2::ThreadPool;
use std::{fs, io};
use std::io::prelude::*;
//use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use std::{net::TcpListener, thread::spawn};
use std::sync::{Arc, Mutex, MutexGuard};
use tungstenite::{accept, Error, handshake::server::{Request, Response}, Message, WebSocket};

struct Point {
  x:u32,
  y:u32
}

struct Player{
  //name: String,
  id : usize,
  color: String,
  posi: Point,
  score: u32
}

struct State {
  players: Vec<Player>,
  map: Vec<Vec<String>>
}

fn main() {
  let listener = TcpListener::bind("127.0.0.1:3012").unwrap();
  let pool = ThreadPool::new(4);

  let mut state = State{ players: vec![], map: vec![] };
  state.map = create_map();
  let current_state = Arc::new(Mutex::new(state));
  let current_state_clone = current_state.clone();
  let mut sockets:Vec<Arc<Mutex<WebSocket<TcpStream>>>> = Vec::new();
  let mut sockets:Arc<Mutex<Vec<Arc<Mutex<WebSocket<TcpStream>>>>>> = Arc::new(Mutex::new(sockets));
  let mut sockets_clone = sockets.clone();
  pool.execute(move || {
    loop {
      //println!("oi");
      let response;
      thread::sleep(Duration::from_millis(9000000));
      {
        let mut state = current_state.clone();
        response = build_response(false, &mut state.lock().unwrap());
      }
      let sockets = sockets_clone.clone();
      let sockets = &*sockets.lock().unwrap();
      for socket in sockets {
        let mut socket = socket.lock().unwrap();
        let _ = (*socket).write_message(Message::Text(response.clone()));
      }
    }
  });
  for stream in listener.incoming() {
    let stream = stream.unwrap();
    stream.set_nonblocking(true).unwrap();
    let mut websocket = accept(stream).unwrap();
    let websocket =  Arc::new(Mutex::new(websocket));
    let current_state = current_state_clone.clone();
    {
      let mut sockets = sockets.clone();
      let mut sockets = sockets.lock().unwrap();
      sockets.push(websocket.clone());
    }
    pool.execute(move || {
      handle_connection(websocket, current_state.clone());
    });
  }

  println!("Shutting down.");
}

fn create_map() -> Vec<Vec<String>> {
  let mut matrix = vec![];
  let mut vector = vec![];
  for _ in 0..10 {
    vector.push("white".to_string());
  }
  for _ in 0..20 {
    matrix.push(vector.clone());
  }
  return matrix;
}

/*
fn handle_connection(mut stream: TcpStream) {
  let mut buffer = [0; 1024];
  stream.read(&mut buffer).unwrap();

  let get = b"GET / HTTP/1.1\r\n";
  let sleep = b"GET /sleep HTTP/1.1\r\n";

  let (status_line, filename) = if buffer.starts_with(get) {
    ("HTTP/1.1 200 OK", "hello.html")
  } else if buffer.starts_with(sleep) {
    thread::sleep(Duration::from_secs(5));
    ("HTTP/1.1 200 OK", "hello.html")
  } else {
    ("HTTP/1.1 404 NOT FOUND", "404.html")
  };

  let contents = fs::read_to_string(filename).unwrap();

  let response = format!(
    "{}\r\nContent-Length: {}\r\n\r\n{}",
    status_line,
    contents.len(),
    contents
  );

  stream.write(response.as_bytes()).unwrap();
  stream.flush().unwrap();
}
*/

fn handle_connection(websocket: Arc<Mutex<WebSocket<TcpStream>>>, current_state: Arc<Mutex<State>>){
  println!("Conectei");
  loop {
    let msg = websocket.lock().unwrap().read_message();
    match msg{
      Ok(conteudo) => {
        let copy_current_state = current_state.clone();
        /*{
            let mut state = current_state.lock().unwrap();
            *state = msg.clone();
        }*/
        _process_message(websocket.clone(), conteudo, copy_current_state);
      }
      Err(Error::Io(ref e)) if e.kind() == io::ErrorKind::WouldBlock => {
        //println!("WouldBlock");
        continue;
      }
      Err(e) => panic!("encountered IO error: {e}")
    }
  }
}

fn _process_message(websocket: Arc<Mutex<WebSocket<TcpStream>>>, message:Message,
                          current_state: Arc<Mutex<State>>){ //responde o cliente
  let msg = message.to_string();
  let info:Vec<&str> = msg.split(";").collect();
  println!("{:?}", info);

  let mut websocket = websocket.lock().unwrap();
  let mut state = current_state.lock().unwrap();
  if info[0]=="conecta" {
    println!("Novo Jogador");
    let jogador = Player{
      id: state.players.len(),
      color: info[2].to_string(),
      posi: Point { x: 5, y: 5 },
      score: 0
    };
    state.players.push(jogador);
    println!("numero de jogadores: {:?}", state.players.len());
  }
  if info[0]=="atualiza" {
    let id = info[1].parse::<usize>().unwrap();
    if info[2] == "cima" {
      state.players[id].posi.y -= 1;
    }
    if info[2] == "baixo" {
      state.players[id].posi.y += 1;
    }
    if info[2] == "esquerda" {
      state.players[id].posi.x -= 1;
    }
    if info[2] == "direita" {
      state.players[id].posi.x += 1;
    }
  }
  else if info[0]=="pinta" {
    let novo_x = info[2].parse::<usize>();
    let novo_y = info[3].parse::<usize>();
    if let Ok(x) = novo_x{
      if let Ok(y) = novo_y {
        state.map[x][y] = "green".to_string();
      }
    }
  }
  for mut jogador in &mut state.players{
    jogador.score = 0;
  }

  for coluna in state.map.clone(){
    for cor in coluna{
      for mut jogador in &mut state.players{
        if jogador.color == *cor {
          jogador.score += 1;
        }
      }
    }
  }

  let ret = build_response(info[0] == "conecta", &mut state);

  let _ = (*websocket).write_message(Message::Text(ret.clone()));
  println!("{:?}", ret);

}

fn build_response(is_connected: bool, state: &mut MutexGuard<State>) -> String {
  let mut ret = String::from("{\"jogadores\" : [");
  let mut count = 0;
  for jogador in &state.players {
    let aaa = format!("{{\"id\":\"{}\" ,\"cor\":\"{}\" , \"x\":{} , \"y\":{} , \"pontua????o\":{}}}",
                      jogador.id, jogador.color, jogador.posi.x, jogador.posi.y, jogador.score);
    ret = ret + &aaa;

    if count < state.players.len() - 1 {
      ret.push(',');
    }
    count = count + 1
  }
  ret = ret + &format!("], \"mapa\":{:?}", state.map);

  if is_connected {
    ret = ret + &format!(",\"id\": {}", state.players.len() - 1);
  }

  ret = ret + "}";
  ret
}