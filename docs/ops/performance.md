# Performance budget y benchmarks

Cómo verificamos que pharma-server cumple el SLA de latencia POS y cómo
detectamos regresiones antes de un release.

> Implementación: `crates/api/benches/pos_sale.rs` (Criterion harness).
> Presupuesto fuente: `CLAUDE.md` § "Performance budget".

## Presupuesto

| Endpoint                  | Target       | Hardware base                |
|---------------------------|--------------|------------------------------|
| `POST /api/v1/pos/sale`   | **<50 ms p99** | i3 + SSD + 8 GB RAM (mínimo) |
| `GET  /api/v1/inventory`  | <50 ms p99   | mismo                        |
| `GET  /api/v1/products?limit=100` | <50 ms p99 | mismo                  |

`POS sale` es **hot path**: bloquea el cajero en pantalla. Cualquier regresión
en esta métrica se considera bug release-blocker.

## Bench harness — qué mide

`crates/api/benches/pos_sale.rs` arma el `Router` completo (axum + middleware
auth/audit/role) sobre **SurrealDB `kv-mem`** y ejecuta `POST /api/v1/pos/sale`
con un ítem, pago efectivo, sin descuentos ni receta — el path feliz que más se
ejecuta en producción.

Por qué `kv-mem` y no `kv-surrealkv` (el motor que va en el MSI):

- Queremos medir **overhead CPU + código** (routing, auth, decimal serde,
  validación, escritura multi-stmt) sin que el fsync del disco contamine la
  varianza.
- `kv-mem` da una cota **inferior** (sin disco no se puede ir más rápido). Si
  excedemos el presupuesto acá, el MSI sobre HDD/SSD lo va a exceder seguro.
- Para la cota **real** sobre hardware mínimo: smoke en una VM Windows con
  perfil release (ver `docs/ops/msi-build.md` y la checklist de release).

Telemetría: el bench **no inicializa `tracing_subscriber`**, así que las
macros `tracing::info!` etc. se compilan a no-op de costo ~0ns. Los flags de
metrics también están desactivados (`metrics_token: None`).

## Cómo correr

Smoke test (verifica que compila y corre, **no toma medidas**):

```powershell
cargo bench -p api --bench pos_sale -- --test
```

Útil en CI o como gate de PR — termina en <30 s y falla si el harness se
rompió (p.ej. el modelo `PosSaleRequest` cambió).

Medición completa (≈30-60 s con todos los grupos):

```powershell
cargo bench -p api --bench pos_sale
```

Salida HTML en `target/criterion/`. Abrir
`target/criterion/pos_sale/single_item_cash_happy_path/report/index.html` para
ver histograma + comparación contra el último run.

Sólo el grupo `pos_sale` (el más crítico):

```powershell
cargo bench -p api --bench pos_sale -- pos_sale
```

Sólo el grupo de lecturas:

```powershell
cargo bench -p api --bench pos_sale -- reads
```

## Cómo leer los números

Criterion reporta media, mediana, y desv. estándar, no p99 directo. Reglas
prácticas para nuestro caso (50 muestras, distribución cercana a normal):

- **media < 25 ms** → margen cómodo, p99 esperado < 50 ms.
- **media 25-40 ms** → revisar p99 manualmente (`target/criterion/.../sample.json`)
  o subir muestras con `--sample-size 200`.
- **media > 40 ms** → regresión probable, **bloquear el release**.

Los benches comparan automáticamente contra el último run guardado (`target/criterion/`).
Un `change: [+5% +12% +20%]` rojo es la señal de alerta.

## Qué hacer si regresa

1. **Reproducir local**:
   ```powershell
   git checkout <commit-bueno>
   cargo bench -p api --bench pos_sale
   git checkout <commit-malo>
   cargo bench -p api --bench pos_sale
   ```
   Criterion guarda el baseline; el segundo run muestra `change: +Xs%`.

2. **Aislar el culpable** con `cargo flamegraph` (se instala aparte —
   `cargo install flamegraph`; en Windows requiere `dtrace` o `wpr.exe` —
   ver [flamegraph-rs/flamegraph](https://github.com/flamegraph-rs/flamegraph)):
   ```powershell
   cargo flamegraph --bench pos_sale --features bench-profile -- --bench
   ```

3. **Bisect** en CI: clonar el repo, hacer `git bisect` corriendo el bench como
   script `bad`/`good`.

4. **Análisis de queries**: si el culpable está en `service::post_sale`,
   revisar las queries SurrealQL — ver `crates/domain/src/sales/repo.rs` y
   `crates/domain/src/inventory/repo.rs`. La regla es: una sola transacción
   multi-stmt, índices por `tenant` (ver `migrations/0001_init.surql`).

## Notas y caveats

- **No usar `--release` en `cargo test`**: el harness se compila con perfil
  bench (≈ release) automáticamente; usar `cargo test --release` mediría el
  build de tests sin Criterion.
- **Variance**: cerrar Chrome / OBS / etc. antes de medir; un solo core a
  100 % de otra app mueve la media >5 %.
- **CI**: por ahora `cargo bench -- --test` sólo verifica que el harness
  compila; los números reales se generan local en hardware estable. Un job
  dedicado de bench en CI vendría con la Fase 9 (release pipeline).
