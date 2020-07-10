use dotenv::dotenv;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres::Client;
use postgres_openssl::MakeTlsConnector;
use regex::Regex;
use std::env;
use std::time::Instant;

#[derive(Debug)]
struct Filas {
    clave: String,
    generacion: String,
    nombre: String,
    apellidos: String,
}

fn main() {
    //     dotenv().ok();
    //     let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
    //     builder.set_verify(SslVerifyMode::NONE);
    //     let connector = MakeTlsConnector::new(builder.build());
    //
    //     let mut client = Client::connect(
    //         &env::var("DATABASE_URL").expect("DATABASE_URL no encontrada"),
    //         connector,
    //     )
    //     .expect("No se pudo conectar a la BDD");
    //
    //     let mut numero_de_filas: i64 = client
    //         .query_one("SELECT count(*) n from bot_claves", &[])
    //         .unwrap()
    //         .get("n");
    //
    //     let mut azules: Vec<Filas> = Vec::with_capacity(numero_de_filas as usize);
    //
    //     numero_de_filas = client
    //         .query_one("SELECT count(*) n from bot_claves", &[])
    //         .unwrap()
    //         .get("n");
    //
    //     let mut internos: Vec<Filas> = Vec::with_capacity(numero_de_filas as usize);
    //
    //     for row in client
    //         .query(
    //             "SELECT clave, generacion, nombre, apellidos from bot_claves",
    //             &[],
    //         )
    //         .unwrap()
    //     {
    //         let clave: String = row.get(0);
    //         let generacion = match row.get(1) {
    //             0 => String::from("N"),
    //             _ => roman::to(row.get(1)).unwrap(),
    //         };
    //         let nombre: String = row.get(2);
    //         let apellidos: String = row.get(3);
    //         //         println!("{} | {} {} {}", clave, generacion, nombre, apellidos);
    //         azules.push(Filas {
    //             clave,
    //             generacion,
    //             nombre,
    //             apellidos,
    //         });
    //     }
    //
    //     for row in client
    //         .query(
    //             "SELECT clave, generacion, nombre, apellidos from bot_internos",
    //             &[],
    //         )
    //         .unwrap()
    //     {
    //         let clave: String = row.get(0);
    //         let generacion = match row.get(1) {
    //             0 => String::from("N"),
    //             _ => roman::to(row.get(1)).unwrap(),
    //         };
    //         let nombre: String = row.get(2);
    //         let apellidos: String = row.get(3);
    //         //         println!("{} | {} {} {}", clave, generacion, nombre, apellidos);
    //         internos.push(Filas {
    //             clave,
    //             generacion,
    //             nombre,
    //             apellidos,
    //         });
    //     }
    //
    //     println!("{:?}, {:?}", azules, internos);

    let now = Instant::now();
    let patron_azules = Regex::new(r"[Aa]\d{3}").unwrap();
    println!("{:?}", Instant::now().duration_since(now));
}
