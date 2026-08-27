-- Modelo da stack em SQLite: uma tabela por coisa que a página tem.
-- A stack é uma só — a da pasta do --dir —, então nenhuma tabela leva id de
-- stack: cada uma guarda as linhas dela e pronto.
-- Rodar de novo num banco pronto não muda nada — é o mesmo caminho da
-- primeira vez e das seguintes.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- O Ambiente: o `DEFAULTS` da página, uma coluna por chave. Linha única, com
-- `id` travado em 1 pelo CHECK — é o que faz a stack ser uma só.
CREATE TABLE IF NOT EXISTS stack_env (
  id        INTEGER PRIMARY KEY CHECK (id = 1),
  restart   TEXT NOT NULL DEFAULT '',
  project   TEXT NOT NULL DEFAULT '',
  cfg       TEXT NOT NULL DEFAULT '',
  data      TEXT NOT NULL DEFAULT '',
  dl        TEXT NOT NULL DEFAULT '',
  http      TEXT NOT NULL DEFAULT '',
  https     TEXT NOT NULL DEFAULT '',
  puid      TEXT NOT NULL DEFAULT '',
  pgid      TEXT NOT NULL DEFAULT '',
  tz        TEXT NOT NULL DEFAULT '',
  -- o idioma da busca (v0.7); vazio nao mexe no idioma de app nenhum
  search_lang TEXT NOT NULL DEFAULT '',
  api_key   TEXT NOT NULL DEFAULT '',
  qbit_user TEXT NOT NULL DEFAULT '',
  qbit_pass TEXT NOT NULL DEFAULT '',
  qbit_key  TEXT NOT NULL DEFAULT '',
  jf_user   TEXT NOT NULL DEFAULT '',
  jf_pass   TEXT NOT NULL DEFAULT '',
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
  key        TEXT PRIMARY KEY,
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
  extra      TEXT NOT NULL DEFAULT '{}'
);

-- As pastas avulsas do Jellyfin, do "+ pasta": lista de caminhos, na ordem.
CREATE TABLE IF NOT EXISTS instance_lib (
  instance_key TEXT NOT NULL REFERENCES instance(key) ON DELETE CASCADE,
  ord          INTEGER NOT NULL,
  path         TEXT NOT NULL,
  PRIMARY KEY (instance_key, ord)
);

-- CONFIG.apps: o que o Prowlarr configura.
CREATE TABLE IF NOT EXISTS cfg_app (
  arr_key TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1
);

-- CONFIG.clients: um cliente de download por linha. As colunas do `cdh` são
-- nulas em quem não tem gerenciamento de downloads concluídos.
CREATE TABLE IF NOT EXISTS cfg_client (
  client_key    TEXT PRIMARY KEY,
  cdh_completed INTEGER,
  cdh_failed    INTEGER
);

-- Quem recebe do cliente, e com que categoria.
CREATE TABLE IF NOT EXISTS cfg_client_arr (
  client_key TEXT NOT NULL REFERENCES cfg_client(client_key) ON DELETE CASCADE,
  arr_key    TEXT NOT NULL,
  enabled    INTEGER NOT NULL DEFAULT 1,
  category   TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (client_key, arr_key)
);

-- CONFIG.mm: Media Management é por família, não por instância — a chave aqui
-- é o id do serviço, não o `cname()`.
CREATE TABLE IF NOT EXISTS cfg_mm (
  service_id TEXT PRIMARY KEY,
  hardlink   INTEGER NOT NULL DEFAULT 1,
  rename     INTEGER NOT NULL DEFAULT 1,
  perms      INTEGER NOT NULL DEFAULT 0,
  empty      INTEGER NOT NULL DEFAULT 0,
  chmod      TEXT NOT NULL DEFAULT '755',
  chown      TEXT NOT NULL DEFAULT ''
);

-- CONFIG.profiles: os perfis de qualidade, por instância — ao contrário do
-- Media Management, aqui a chave é o `cname()`: é justamente por instância que
-- eles diferem (4K numa, anime na outra). Os presets vão em JSON, como o
-- `cfg_naming` faz com o valor: é uma lista, e uma tabela a mais para guardar
-- meia dúzia de nomes não pagaria o próprio custo.
CREATE TABLE IF NOT EXISTS cfg_profile (
  arr_key TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1,
  presets TEXT NOT NULL DEFAULT '[]',
  extra   TEXT NOT NULL DEFAULT ''
);

-- A nomenclatura, campo a campo. O valor vai em JSON porque o `NAMING_FIELDS`
-- mistura caixa de seleção com formato de texto.
CREATE TABLE IF NOT EXISTS cfg_naming (
  service_id TEXT NOT NULL REFERENCES cfg_mm(service_id) ON DELETE CASCADE,
  field      TEXT NOT NULL,
  value      TEXT NOT NULL,
  PRIMARY KEY (service_id, field)
);
