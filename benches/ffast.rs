use std::{
    collections::HashMap,
    env,
    time::{Duration, Instant},
};
use tokio_postgres::{Error, NoTls};

const N: u32 = 1 << 22;

#[tokio::main]
async fn main() -> Result<(), Error> {
    for i in 0..10 {
        let now = Instant::now();
        let (client, connection) = tokio_postgres::connect(
            &env::var("DATABASE").expect("Base de datos no encontrada o mal configurada."),
            NoTls,
        )
        .await?;

        tokio::spawn(async move {
            connection.await.expect("Conexión fallida.");
        });

        let numero_de_registros = client
            .query_one(
                "SELECT ((SELECT COUNT(*) FROM bot_claves) + (SELECT COUNT(*) FROM bot_internos))",
                &[],
            )
            .await?;

        let mut map = HashMap::with_capacity(numero_de_registros.get::<usize, i64>(0) as usize);

        for row in client
            .query(
                "SELECT clave, generacion, nombre, apellidos 
            FROM (SELECT * from bot_claves UNION SELECT * from bot_internos)x;",
                &[],
            )
            .await?
            .iter()
        {
            map.insert(
                row.get::<usize, &str>(0).to_uppercase(),
                Filas {
                    clave: row.get(0),
                    generacion: match row.get(1) {
                        0 => String::from("N"),
                        _ => roman::to(row.get(1))
                            .expect("Error al convertir generación a número romano"),
                    },
                    nombre: row.get(2),
                    apellidos: row.get(3),
                },
            );
        }

        eprintln!("Datos listos: {:#?}", Instant::now().duration_since(now));

        let mut mx = 0.0f64;
        let mut mn = 10000.0f64;
        let mut sum = Duration::new(0, 0);

        for i in 0..N {
            let t = Instant::now();
            map.get(&format!("A{}", &(i % 400)));
            let took = t.elapsed();
            mx = mx.max(took.as_secs_f64());
            mn = mn.min(took.as_secs_f64());
            sum += took;
        }

        eprintln!(
            "hashbrown: max: {:?}, min: {:?}, mean: {:?}",
            Duration::from_secs_f64(mx),
            Duration::from_secs_f64(mn),
            sum / N
        );
    }

    Ok(())
}

struct Filas {
    clave: String,
    generacion: String,
    nombre: String,
    apellidos: String,
}
