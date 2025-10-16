# nightstand-rs (en Desarrollo)

Pequeño experimento en Rust para manejar un nightstand LED basado en ESP32. Usa el HAL de ESP-IDF para hablar con un strip WS2812 y prepara espacios para tareas de Wi-Fi.

## Requisitos

- `rustup` con toolchain nightly (el repositorio incluye `rust-toolchain.toml`).
- `cargo` y `rust-src` (se instala automáticamente por `rust-toolchain.toml`).
- Opcional: `cargo-espflash` para flashear y monitorizar el dispositivo: `cargo install cargo-espflash`.

## Preparar entorno

```bash
# activar toolchain nightly indicada en rust-toolchain.toml
rustup show active-toolchain

# añadir objetivo si compilas para ESP32 clásico
rustup target add xtensa-esp32-espidf --toolchain nightly
```

## Construir

Compila el binario del proyecto (perfil debug por defecto):

```bash
cargo build
```

Para un binario optimizado:

```bash
cargo build --release
```

Los artefactos quedan en `target/{debug,release}/nightstand-rs`.

## Flashear y monitorear

Conecta el ESP32 por USB y usa `cargo-espflash` para cargar y abrir la consola serie.

```bash
cargo espflash flash --monitor /dev/ttyUSB0
```

Ajusta el puerto según tu sistema (`COMx` en Windows, `/dev/cu.usbserial*` en macOS).

## Estructura del código

- `src/main.rs`: configura logging, toma los periféricos y lanza las tareas (`ws2812_task`, `wifi_task`, `blink_task`).
- `src/ws2812.rs`: lógica para generar señales RMT que controlan el strip WS2812 (timings, buffer y transmisión).

## Desarrollo

- Formatear: `cargo fmt`
- Revisar warnings/errores: `cargo check`
- Clippy (opcional, requiere toolchain nightly con `clippy`): `cargo clippy`

## Roadmap breve

- Completar la tarea Wi-Fi y credenciales.
- Exponer animaciones más complejas para el strip LED.
- Integrar lectura de sensores / interacción tactil del nightstand.
