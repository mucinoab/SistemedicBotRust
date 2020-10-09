use std::{
    collections::BTreeMap,
    env,
    fmt::Write,
    thread,
    time::{Duration, Instant},
};

use async_compat::Compat;
use deunicode::deunicode;
use once_cell::sync::Lazy;
use postgres::{Client, NoTls};
use regex::RegexSet;
use smallvec::SmallVec;
use smartstring::alias::String;
use smol::prelude::*;
use telegram_bot::{types::Message, Api, CanSendMessage, MessageKind, UpdateKind};

#[macro_use]
extern crate log;

fn main() {
    log4rs::init_file("log_config.yml", Default::default()).expect("No se pudo iniciar Log");
    info!("Iniciando...");

    let mut client = Client::connect(
        &env::var("DATABASE").expect("Variable de base de datos no encontrada."),
        NoTls,
    )
    .expect("PostgreSQL no encontrada o mal configurada.");

    let mut map: BTreeMap<String, Persona> = BTreeMap::new();
    client
        .query(
            "SELECT * FROM bot_claves UNION SELECT * FROM bot_internos;",
            &[],
        )
        .expect("Extraer datos de BDD")
        .iter()
        .for_each(|row| {
            map.insert(
                String::from(row.get::<usize, &str>(0).trim()),
                Persona {
                    generacion: row.get::<usize, i32>(1) as i8,
                    nombre: String::from(row.get::<usize, &str>(2).trim()),
                    apellidos: String::from(row.get::<usize, &str>(3).trim()),
                },
            );
        });

    let mut texto = std::string::String::with_capacity(348);
    let mut buscados: Vec<String> = Vec::with_capacity(1);
    let mut generaciones = SmallVec::<[i8; 2]>::new();
    let mut encontrado = false;
    Lazy::force(&RE);

    let api = Api::new(&env::var("TOKEN").expect("Token no encontrado"));
    let mut stream = api.stream();

    info!("Datos procesados, listo para recibir querys.");

    smol::block_on(Compat::new(async {
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
                                            map.get_key_value(cap.to_uppercase().as_str())
                                        {
                                            writeln!(
                                                texto,
                                                "{}  {} {}, gen {}",
                                                clave,
                                                persona.nombre,
                                                persona
                                                    .apellidos
                                                    .split_whitespace()
                                                    .next()
                                                    .unwrap_or_default(),
                                                roman::to(persona.generacion as _).unwrap_or_else(
                                                    || std::string::String::from("N")
                                                ),
                                            )
                                            .unwrap();
                                        }
                                    }
                                }

                                Comando::Nombre => {
                                    buscados = data
                                        .split_whitespace()
                                        .skip(1)
                                        .filter_map(|palabra| {
                                            if palabra.len() > 2 {
                                                Some(String::from(
                                                    deunicode(palabra).to_lowercase(),
                                                ))
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();

                                    for (clave, persona) in &map {
                                        let nombre = deunicode(&persona.nombre).to_lowercase();

                                        encontrado = buscados.iter().any(|nombre_buscado| {
                                            nombre.contains(nombre_buscado.as_str())
                                        });

                                        if encontrado {
                                            writeln!(texto, "{}  {}", clave, persona.apellidos)
                                                .unwrap_or_default();
                                        }
                                    }
                                }

                                Comando::Apellido => {
                                    buscados = data
                                        .split_whitespace()
                                        .skip(1)
                                        .filter_map(|palabra| {
                                            if palabra.len() > 2 {
                                                Some(String::from(
                                                    deunicode(palabra).to_lowercase(),
                                                ))
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();

                                    for (clave, persona) in &map {
                                        let apellidos =
                                            deunicode(&persona.apellidos).to_lowercase();

                                        encontrado = buscados.iter().any(|apellido_buscado| {
                                            apellidos.contains(apellido_buscado.as_str())
                                        });

                                        if encontrado {
                                            writeln!(texto, "{}  {}", clave, persona.nombre)
                                                .unwrap_or_default();
                                        }
                                    }
                                }

                                Comando::Generacion => {
                                    data.split_whitespace().skip(1).for_each(|palabra| {

                                    if let Ok(numero) = palabra.parse::<i8>() {
                                                if numero > 15 && numero < 34 {
                                                    generaciones.push(numero);
                                                } else {
                                                    writeln!(
                                                        texto,
                                                        "No tengo datos sobre quien pertenece a la generación {} :(", numero
                                                    )
                                                    .unwrap_or_default();
                                                }
                                        }
                                    });

                                    for (clave, datos) in &map {
                                        if generaciones.iter().any(|n| *n == datos.generacion) {
                                            writeln!(
                                                texto,
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
                            generaciones.clear();
                            buscados.clear();
                        }
                    }
                }

                Err(e) => {
                    error!("{}", e);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }));
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
    generacion: i8,
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

impl From<&std::string::String> for Comando {
    fn from(item: &std::string::String) -> Self {
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
/generacion 32
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
/g 32 33
/clave a101 A007 A010*
/c A342 A225 cKGr
/n Sam Pedro
/a castillo

Código Fuente: https://github.com/mucinoab/SistemedicBotRust"#;

static NO_ENTIENDO: &str = "No te entendí...\nIntenta de nuevo o usa \"/h\" para ayuda.";

static NO_ENCONTRE: &str = "No encontré lo que buscas...\nIntenta de nuevo.";
