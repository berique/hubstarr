-- Modelo da stack em SQLite: uma tabela por coisa que a página tem.
-- Rodar de novo num banco pronto não muda nada — é o mesmo caminho da
-- primeira vez e das seguintes.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- Uma stack por linha; `dir` é a pasta em que os arquivos gerados são gravados.
CREATE TABLE IF NOT EXISTS stack (
  id         INTEGER PRIMARY KEY,
  slug       TEXT NOT NULL UNIQUE,
  name       TEXT NOT NULL,
  dir        TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- O Ambiente: o `DEFAULTS` da página, uma coluna por chave.
CREATE TABLE IF NOT EXISTS stack_env (
  stack_id  INTEGER PRIMARY KEY REFERENCES stack(id) ON DELETE CASCADE,
  restart   TEXT NOT NULL DEFAULT '',
  cfg       TEXT NOT NULL DEFAULT '',
  data      TEXT NOT NULL DEFAULT '',
  dl        TEXT NOT NULL DEFAULT '',
  http      TEXT NOT NULL DEFAULT '',
  https     TEXT NOT NULL DEFAULT '',
  puid      TEXT NOT NULL DEFAULT '',
  pgid      TEXT NOT NULL DEFAULT '',
  tz        TEXT NOT NULL DEFAULT '',
  api_key   TEXT NOT NULL DEFAULT '',
  qbit_user TEXT NOT NULL DEFAULT '',
  qbit_pass TEXT NOT NULL DEFAULT '',
  qbit_key  TEXT NOT NULL DEFAULT '',
  tls       INTEGER NOT NULL DEFAULT 0,
  domain    TEXT NOT NULL DEFAULT '',
  cert      TEXT NOT NULL DEFAULT '',
  tls_key   TEXT NOT NULL DEFAULT '',
  vpn_prov  TEXT NOT NULL DEFAULT '',
  vpn_type  TEXT NOT NULL DEFAULT '',
  wg_key    TEXT NOT NULL DEFAULT '',
  wg_addr   TEXT NOT NULL DEFAULT '',
  ovpn_user TEXT NOT NULL DEFAULT '',
  ovpn_pass TEXT NOT NULL DEFAULT '',
  countries TEXT NOT NULL DEFAULT ''
);

-- Uma linha por serviço adicionado. A chave é o `cname()` da página — o
-- container_name —, a mesma que a Configuração usa para se referir a ele.
-- O `ord` guarda a ordem da lista, que é a ordem no compose. O que a página
-- inventar depois cai em `extra`, o resto do objeto em JSON: assim uma flag
-- nova no `SERVICES` volta inteira sem exigir migração.
CREATE TABLE IF NOT EXISTS instance (
  stack_id   INTEGER NOT NULL REFERENCES stack(id) ON DELETE CASCADE,
  key        TEXT NOT NULL,
  ord        INTEGER NOT NULL DEFAULT 0,
  service_id TEXT NOT NULL,
  title      TEXT NOT NULL DEFAULT '',
  data       TEXT NOT NULL DEFAULT '',
  abs        TEXT NOT NULL DEFAULT '',
  hw         TEXT NOT NULL DEFAULT 'cpu',
  tpv        TEXT NOT NULL DEFAULT 'std',
  tpt        TEXT NOT NULL DEFAULT 'organizr',
  vpn        INTEGER NOT NULL DEFAULT 0,
  solver     INTEGER NOT NULL DEFAULT 0,
  extra      TEXT NOT NULL DEFAULT '{}',
  PRIMARY KEY (stack_id, key)
);

-- As pastas avulsas do Jellyfin, do "+ pasta": lista de caminhos, na ordem.
CREATE TABLE IF NOT EXISTS instance_lib (
  stack_id     INTEGER NOT NULL,
  instance_key TEXT NOT NULL,
  ord          INTEGER NOT NULL,
  path         TEXT NOT NULL,
  PRIMARY KEY (stack_id, instance_key, ord),
  FOREIGN KEY (stack_id, instance_key)
    REFERENCES instance(stack_id, key) ON DELETE CASCADE
);

-- CONFIG.apps: o que o Prowlarr configura.
CREATE TABLE IF NOT EXISTS cfg_app (
  stack_id INTEGER NOT NULL REFERENCES stack(id) ON DELETE CASCADE,
  arr_key  TEXT NOT NULL,
  enabled  INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (stack_id, arr_key)
);

-- CONFIG.clients: um cliente de download por linha. As colunas do `cdh` são
-- nulas em quem não tem gerenciamento de downloads concluídos.
CREATE TABLE IF NOT EXISTS cfg_client (
  stack_id      INTEGER NOT NULL REFERENCES stack(id) ON DELETE CASCADE,
  client_key    TEXT NOT NULL,
  cdh_completed INTEGER,
  cdh_failed    INTEGER,
  PRIMARY KEY (stack_id, client_key)
);

-- Quem recebe do cliente, e com que categoria.
CREATE TABLE IF NOT EXISTS cfg_client_arr (
  stack_id   INTEGER NOT NULL,
  client_key TEXT NOT NULL,
  arr_key    TEXT NOT NULL,
  enabled    INTEGER NOT NULL DEFAULT 1,
  category   TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (stack_id, client_key, arr_key),
  FOREIGN KEY (stack_id, client_key)
    REFERENCES cfg_client(stack_id, client_key) ON DELETE CASCADE
);

-- CONFIG.mm: Media Management é por família, não por instância — a chave aqui
-- é o id do serviço, não o `cname()`.
CREATE TABLE IF NOT EXISTS cfg_mm (
  stack_id   INTEGER NOT NULL REFERENCES stack(id) ON DELETE CASCADE,
  service_id TEXT NOT NULL,
  hardlink   INTEGER NOT NULL DEFAULT 1,
  rename     INTEGER NOT NULL DEFAULT 1,
  perms      INTEGER NOT NULL DEFAULT 0,
  empty      INTEGER NOT NULL DEFAULT 0,
  chmod      TEXT NOT NULL DEFAULT '755',
  chown      TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (stack_id, service_id)
);

-- A nomenclatura, campo a campo. O valor vai em JSON porque o `NAMING_FIELDS`
-- mistura caixa de seleção com formato de texto.
CREATE TABLE IF NOT EXISTS cfg_naming (
  stack_id   INTEGER NOT NULL,
  service_id TEXT NOT NULL,
  field      TEXT NOT NULL,
  value      TEXT NOT NULL,
  PRIMARY KEY (stack_id, service_id, field),
  FOREIGN KEY (stack_id, service_id)
    REFERENCES cfg_mm(stack_id, service_id) ON DELETE CASCADE
);
