use std::{
    collections::BTreeMap,
    env,
    fmt::Write,
    thread,
    time::{Duration, Instant},
};

use deunicode::deunicode;
use futures::StreamExt;
use once_cell::sync::Lazy;
use regex::RegexSet;
use telegram_bot::*;
use tokio_postgres::{Error, NoTls};

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<(), Error> {
    log4rs::init_file("log_config.yml", Default::default()).expect("No se pudo iniciar Log");
    info!("Iniciando...");

    let (client, connection) = tokio_postgres::connect(
        &env::var("DATABASE").expect("Base de datos no encontrada o mal configurada."),
        NoTls,
    )
    .await?;

    tokio::spawn(async move {
        connection.await.expect("Conexión a base de datos fallida.");
    });

    let map: BTreeMap<String, Persona> = client
        .query(
            "SELECT * FROM bot_claves UNION SELECT * FROM bot_internos;",
            &[],
        )
        .await?
        .iter()
        .map(|row| {
            (
                row.get::<usize, &str>(0).to_uppercase(),
                Persona {
                    generacion: row.get(1),
                    nombre: row.get(2),
                    apellidos: row.get(3),
                },
            )
        })
        .collect();

    let mut texto = String::with_capacity(348);
    Lazy::force(&RE);

    let api = Api::new(&env::var("TOKEN").expect("Token no encontrado"));
    let mut stream = api.stream();

    info!("Datos procesados, listo para recibir querys.");
    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                if let UpdateKind::Message(message) = update.kind {
                    if let MessageKind::Text { ref data, .. } = message.kind {
                        let now = Instant::now();

                        match Comando::from(data) {
                            Comando::Clave => {
                                for cap in data.split_whitespace().skip(1) {
                                    if let Some((clave, persona)) =
                                        map.get_key_value(&cap.to_uppercase())
                                    {
                                        writeln!(
                                            &mut texto,
                                            "{}  {} {}, gen {}",
                                            clave,
                                            persona.nombre,
                                            persona
                                                .apellidos
                                                .split_whitespace()
                                                .next()
                                                .unwrap_or_default(),
                                            roman::to(persona.generacion)
                                                .unwrap_or_else(|| String::from("N")),
                                        )
                                        .unwrap();
                                    }
                                }
                            }

                            Comando::Nombre => {
                                let nombres_buscados: Vec<String> = data
                                    .split_whitespace()
                                    .skip(1)
                                    .filter_map(|palabra| {
                                        if palabra.len() > 2 {
                                            Some(deunicode(palabra).to_lowercase())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();

                                for (clave, persona) in &map {
                                    let nombre = deunicode(&persona.nombre).to_lowercase();

                                    for nombre_buscado in &nombres_buscados {
                                        if nombre.contains(nombre_buscado) {
                                            writeln!(
                                                &mut texto,
                                                "{}  {}",
                                                clave, persona.apellidos
                                            )
                                            .unwrap_or_default();
                                        }
                                    }
                                }
                            }

                            Comando::Apellido => {
                                let apellidos_buscados: Vec<String> = data
                                    .split_whitespace()
                                    .skip(1)
                                    .filter_map(|palabra| {
                                        if palabra.len() > 2 {
                                            Some(deunicode(palabra).to_lowercase())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();

                                for (clave, persona) in &map {
                                    let apellidos = deunicode(&persona.apellidos).to_lowercase();

                                    for apellido_buscado in &apellidos_buscados {
                                        if apellidos.contains(apellido_buscado) {
                                            writeln!(&mut texto, "{}  {}", clave, persona.nombre)
                                                .unwrap_or_default();
                                        }
                                    }
                                }
                            }

                            Comando::Generacion => {
                                let generaciones: Vec<i32> =
                                    data.split_whitespace().skip(1).filter_map(|palabra| {
                                        match palabra.parse::<i32>() {
                                            Ok(numero) => {
                                                if numero > 15 && numero < 34 {
                                                    Some(numero)
                                                } else {
                                                    writeln!(
                                                        &mut texto,
                                                        "No tengo datos sobre quien pertenece a la generación {} :(", numero
                                                    )
                                                    .unwrap_or_default();

                                                    None
                                                }
                                            }
                                            _ => None,
                                        }
                                    }).collect();

                                for (clave, datos) in &map {
                                    if generaciones.iter().any(|n| *n == datos.generacion) {
                                        writeln!(
                                            &mut texto,
                                            "{}  {} {}",
                                            clave,
                                            datos.nombre,
                                            datos
                                                .apellidos
                                                .split_whitespace()
                                                .next()
                                                .unwrap_or_default()
                                        )
                                        .unwrap_or_default();
                                    }
                                }
                            }

                            Comando::Ayuda => {
                                texto.push_str(AYUDA);
                            }

                            Comando::Start => {
                                texto.push_str(START);
                            }

                            Comando::None => {
                                texto.push_str(NO_ENTIENDO);
                            }
                        };

                        responde(&texto, &message, &api);

                        info!(
                            "{}: {:#?} {:#?}",
                            &message.from.first_name,
                            &data,
                            Instant::now().duration_since(now)
                        );

                        texto.clear();
                    }
                }
            }

            Err(e) => {
                error!("{}", e);
                thread::sleep(Duration::from_millis(1000));
            }
        }
    }

    Ok(())
}

fn responde(texto: &str, message: &Message, api: &Api) {
    if texto.is_empty() {
        api.spawn(message.chat.text(NO_ENCONTRE));
    } else {
        api.spawn(message.chat.text(texto));
    }
}

struct Persona {
    nombre: String,
    apellidos: String,
    generacion: i32,
}

#[derive(Clone, Copy)]
enum Comando {
    Clave,
    Nombre,
    Apellido,
    Generacion,
    Ayuda,
    Start,
    None,
}

impl From<&String> for Comando {
    fn from(item: &String) -> Self {
        let matches = RE.matches(item);

        for (index, comando) in [
            Self::Clave,
            Self::Nombre,
            Self::Apellido,
            Self::Generacion,
            Self::Ayuda,
            Self::Start,
        ]
        .iter()
        .enumerate()
        {
            if matches.matched(index) {
                return *comando;
            }
        }

        Self::None
    }
}

static RE: Lazy<RegexSet> =
    Lazy::new(|| RegexSet::new(&["/[cC]", "/[nN]", "/[aA]", "/[gG]", "/[hH]", "/[sS]"]).unwrap());

static START: &str = r#"Para buscar...
- Clave usa /clave más las claves. 
- Generación entera /generacion más la generación.
- Nombre usa /nombre más los nombres.
- Apellido usa /apellido más los apellidos.

Búsquedas incluyen azules e internos. 

Ejemplo
/clave A101 A027 A010* cKGr 
/nombre Luis Karol
/generacion 33 32
/apellido Soriano

Para ayuda usa /help
Comparte con https://t.me/sistemedicbot"#;

static AYUDA: &str = r#"Para buscar por... 
- Clave usa /clave ó /c más las claves.
- Nombre usa /nombre ó /n más los nombres.
- Apellido usa /apellido ó /a más los apellidos.
- Generaciones enteras usa /generacion ó /g más las generaciones. 
Búsquedas incluyen azules e internos. 

Ejemplo
/g 32 33 19
/clave a101 A007 A010*
/c A342 A225 cKGr
/n Sam Pedro
/a castillo

Código Fuente: https://github.com/mucinoab/SistemedicBotRust"#;

static NO_ENTIENDO: &str = "No te entendí...\nIntenta de nuevo o usa \"/h\" para ayuda.";

static NO_ENCONTRE: &str = "No encontré lo que buscas...\nIntenta de nuevo.";
