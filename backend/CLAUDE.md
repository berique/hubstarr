# CLAUDE.md (backend/)

Guia do servidor Rust deste diretório. O guia geral do projeto — a página
`hubstarr.html`, o catálogo de serviços, os invariantes que a página preserva —
está em `../CLAUDE.md`; carregue-o também quando a mudança tocar os dois lados
do contrato descrito ali (a página gera, o servidor nunca gera conteúdo).

Crate em Rust (axum + rusqlite `bundled`), com a página embutida por
`include_str!` — o cargo rastreia o `hubstarr.html` (ele está no
`target/debug/hubstarr.d`), então mexer na página recompila o binário.

**Quem está servindo a página é o binário, não o arquivo.** Um servidor no ar
entrega a cópia congelada no momento em que foi compilado, e é fácil haver mais
de um por perto — outro clone do repositório, um binário solto no `$HOME`. Antes
de concluir que uma mudança "não pegou", descubra qual responde: o `api/health`
devolve o `dir` e o `db` dele, e um `grep` de alguma marca recente na página
servida a data. E cuidado com a armadilha da porta: subir um segundo servidor
no mesmo endereço falha com `AddrInUse` **em segundo plano** — o processo novo
morre, o velho continua respondendo, e tudo parece só não ter mudado. A saída
dele diz; é a primeira coisa a olhar. `cargo test` roda os testes do modelo e da gravação de arquivos;
`cargo run` serve tudo em `127.0.0.1:7878`. Opções: `--addr`, `--dir` (padrão
`./stack`, a pasta em que os arquivos são gravados), `--db` (padrão
`~/.hubstarr/hubstarr.db`), `--docker`, `-v`.

O que ele escreve vai para a saída **e** para o `server.log`, ao lado do
banco — não na pasta da stack: o log é do servidor, e o `--dir` se apaga e se
refaz enquanto o `--db` dura. É `append`, nunca reescrita, porque o valor dele
é justamente o histórico entre reinícios; e o arquivo que não abre vira um
aviso na saída, não um servidor que não sobe. Quem cuida disso é o
`journal.rs`, e `println!` fora dele é sinal de linha que não vai ao arquivo.

Ali moram **duas alturas de log**, e a distinção é o que mantém as duas úteis:

- `record()` é o que sempre sai — a subida, o motor escolhido, cada
  `PUT /api/settings`. Curto o bastante para se ler dias depois; responde "o que
  mudou na minha stack?".
- `detail()` só existe com o **`-v`**, e é o passo a passo: cada arquivo
  gravado (o `files.rs` e as chaves que o `patch.rs` escreve na conf do app),
  cada linha mexida no banco (instância, Ambiente, Configuração, a lista de
  chaves) e **cada chamada às APIs dos apps**, com método, caminho e status.
  Responde "por que isso não funcionou?". Ligá-lo por padrão afogaria o
  primeiro: uma volta do Aplicar são dezenas de chamadas.

Duas regras do `detail()`: o argumento é uma **função**, para que sem o `-v`
nem o texto seja montado — dá para chamá-lo dentro de laço sem pensar; e **nada
de valor sensível na linha**. O Ambiente sai como lista de *nomes* de campo (a
chave da stack e as senhas estão entre os valores), e a URL sai pelo
`without_query()` onde a query leva a API key. Chamada nova de API deve passar pelo
`api()` do `apply.rs`, que é o único lugar que formata essa linha.

E há **dois destinos**, que é o que decide o idioma de cada linha. O log do
servidor — o `record()`, o `detail()`, o `--help` e as strings de erro internas
— é **inglês**, o idioma do código: quem o lê está lendo o `server.log` ao
lado do banco. Já o que vai para o **modal do log** é lido por quem está na
página, no idioma dela, e por isso não é texto: é o `Msg` do `msg.rs`, um par
`{key, args}` que viaja em JSON e só vira frase na página, no `I18N`
(`job.*`) — o `msgText()` a resolve, e um argumento pode ser outro `Msg`, o que
faz "Sonarr → SABnzbd: <o motivo>" sair de dois templates sem o Rust segurar
prosa traduzida de nenhuma das metades. **A frase mora num lugar só**, e é a
página; escrever texto para o usuário no Rust é o erro que isso previne.

A saída de emergência é o `Msg::raw`, para texto que não tem template próprio
— a mensagem de validação de outro app, um erro do sistema operacional —,
mostrado como veio. `grep -rn 'Msg::raw' src/` devolvendo pouca coisa é o
sinal de que está sendo usado assim; chave nova pede a linha nos **três**
idiomas, e o `check-i18n.py` cobra.

**A stack é uma só**, a da pasta do `--dir`: nenhum caminho da API leva id e
nenhuma tabela tem `stack_id`. Manter duas é rodar dois servidores, cada um com
o seu `--dir` e o seu `--db`. Já houve seletor de stack no cabeçalho, com
`POST`/`DELETE /api/stacks`, e foi removido; não refaça o caminho de volta.

O banco daquela versão é migrado na abertura, pelo `store/migrate.rs`, que roda
**antes** do `schema.sql` — o `CREATE TABLE IF NOT EXISTS` não mexe em tabela
que já existe, então as antigas são renomeadas para `old_*`, o esquema novo
nasce ao lado e a stack de menor id é copiada para dentro dele. As outras se
perdem, de propósito: não há mais onde guardá-las, e o `dir` de cada uma é
anunciado na saída. Duas armadilhas do SQLite ali: o `legacy_alter_table` tem
de estar ligado para o `RENAME` não reescrever as chaves estrangeiras das
outras tabelas, e o `foreign_keys` tem de ser desligado *depois* do
`schema.sql`, que o religa — senão nem o `SELECT` das `old_*` nem o `DROP`
delas passam.

**O contrato, que é o que quase não muda: nenhum arquivo da stack nasce no
servidor.** Ele recebe pronto o que os geradores do `<script>` montaram
(`outFiles()`), grava e roda o `docker compose`. Os geradores existem num lugar
só — se você for tentado a montar YAML no Rust, é sinal de que a mudança
pertence à página.

A **única** exceção é o `apply.rs` do v0.3, e ela tem limite claro: um cliente
de download não é arquivo — o *arr guarda isso no banco dele e só aceita pela
API —, então ali o servidor monta o corpo JSON do `downloadclient`. O que ele
monta é só o **formato da API do app** (implementação, contrato, a lista de
`fields`). Decisão nenhuma é dele: endereço, porta, categoria, quem recebe o
quê e o nome do campo de categoria de cada família chegam prontos no corpo do
`POST`, do `applyPayload()` da página, que é onde o `SERVICES` e o `CONFIG`
vivem. Ao acrescentar o Prowlarr ou o Media Management, siga essa divisão.

Módulos: `store/` (o modelo, com `migrate.rs` à parte), `files.rs` (grava o que veio, com `safe_join()`
recusando o que escapa da pasta, e o `ensure_dirs()` que cria as pastas dos
`source` do compose antes do `up` — o Docker criaria as que faltam, mas como
**root**, e aí o app, rodando com o PUID/PGID do Ambiente, não escreve na
própria configuração; quem lista as pastas é a página, no `outDirs()`, porque é
ela que monta os caminhos), `deploy.rs` (`docker compose up -d`/`down` na
pasta da stack, mais o `docker_ok()` que o `api/health` devolve — ele pergunta
por `docker compose version`, não só pelo docker, porque o plugin é pacote à
parte e é ele que sobe a stack; sem ele a página abre o bloco "Precisa instalar
o Docker?" e mostra o aviso `#noDocker`. Em quem esse teste é feito sai do
`pick_engine()`, resolvido uma vez na subida: o comando do `--docker`, se veio,
senão o primeiro dos `ENGINES` (`docker`, `podman`) que passar — o
`podman compose` roda o mesmo arquivo, e máquina que só tem ele não tem docker
nenhum a encontrar; o `up_one`/`stop_one` é um container só, o que o clique no
ponto de status da lista chama por `POST /api/service/:key/:action` — `up` sobe
com `--no-deps`, para não arrastar vizinho parado, e `down` é um `stop`, que
deixa o container existindo em vez de sumir. A chave vira argumento de comando,
então passa pelo `ok_service()` antes), `apply.rs` (v0.3: a Configuração inteira
aplicada pela API — clientes de download em cada *arr **e no próprio Prowlarr**,
cada *arr no Prowlarr, o Media Management mais a nomenclatura de cada família, e
as **pastas raiz** de cada instância.

A pasta raiz é o caminho **de dentro do container** (`/data` mais a subpasta da
instância), e quem o monta é a página, no `rootFolders()` — é ela que escreve os
binds do compose, então é ela que sabe o que o app enxerga. Mandar o caminho do
host não dá erro na hora: o *arr aceita e depois não acha arquivo nenhum. O
`ensure_root_folders()` acrescenta o que falta e **não tira** o que já está lá — remover
pasta raiz leva a biblioteca junto.

O **Lidarr** pede mais do que o caminho, e isso foi medido no app: o `Name` não
pode ser vazio e os dois perfis padrão (`defaultQualityProfileId` e
`defaultMetadataProfileId`) têm de ser maiores que zero — com só o `path`, a
resposta é uma lista de validação e a pasta não nasce. O nome sai do
`LIDARR_ROOT_NAME` (**`Music`**; nome repetido ele aceita, então duas pastas de
música não precisam de desempate) e os ids saem do `first_id()`, que lê a
lista do próprio app e cai no `1` — o de fábrica — quando não dá para ler.
Sonarr e Radarr não têm esses campos, então o ramo é só do Lidarr.

O **Jellyfin** é a única volta que não fala a API dos *arr: sem `X-Api-Key`,
com requisições próprias, e é o `StartupWizardCompleted` do
`/System/Info/Public` que escolhe o caminho. Assistente aberto é a janela em
que o `FirstTimeSetupOrElevated` dele aceita criar usuário e biblioteca **sem
token** — daí a ordem, com as bibliotecas *antes* do `Startup/Complete`.
Assistente fechado exige token, e ele sai do usuário e senha do modal
(`webAuth: 'jf'`); sem eles, é uma linha no log, não uma falha da stack. O
`Complete` só é chamado quando houve administrador a criar: fechar o assistente
sem conta nenhuma entrega um Jellyfin em que ninguém entra. Biblioteca que já
existe não é tocada e nenhuma é removida, pela mesma razão da pasta raiz. Quem
monta a lista é a página, no `jellyfinLibs()`, com o caminho **de dentro do
container** — e ele **não** é a pasta raiz do *arr: o Jellyfin monta a base
inteira em `/data` mais uma pasta por caminho de fora dela, e é contra esses
binds que a biblioteca tem de bater. Caminho que ele não enxerga não dá erro: a
biblioteca nasce vazia e calada, e é por isso que o `check-compose.py` a
confere contra os binds do compose.

Com o FlareSolverr na stack, ele também entra no Prowlarr, em Settings →
Indexers → Indexer Proxies, com a **etiqueta `flaresolverr`** — criada ali
mesmo, se não existir. A etiqueta não é enfeite: o Prowlarr casa proxy com
indexador por ela, e escolher quais indexadores precisam do resolvedor é a parte
que fica com quem usa. O endereço é o interno (`http://<cname>:8191`), porque o
serviço é `internal` e não tem rota no nginx; o nome do registro é o título da
instância, então a stack que roda o Byparr com outro nome aparece com ele.

O **qBittorrent** recebe ainda as preferências dele pela API, no
`client_preferences()`: o `app/setPreferences` com o corpo que a página
monta no `qbitPrefs()` — o gerenciamento automático de torrent
(`auto_tmm_enabled` e `torrent_changed_tmm_enabled`, que é o que faz o torrent
seguir a categoria quando ela muda), o `save_path` de dentro do container e o
usuário e a senha da interface. Não é repetição da conf do `patch.rs`: aquela é
o que ele lê ao **nascer**, esta é a mesma decisão aplicada a um qBittorrent que
já existe — e o TMM a conf nem cobre. O caminho sai do `qbitDl()`, o mesmo dos
dois lugares, para conf e API não discordarem.

A **API key** é do app, não nossa: a conf só a recebe quando ele ainda não tem
uma. Quem faz isso é o `keep_keys` do `patch.rs` — a lista de chaves que o merge
**não** sobrescreve quando o arquivo já traz valor —, e a página manda
`keep:['WebUI\\APIKey']` no patch do qBittorrent. Vazia ou ausente ela é
escrita, que é a primeira subida. A razão: uma vez que o app responda por uma
chave, é ela que os clientes dele conhecem, e trocá-la a cada Subir cortaria
quem já falava com ele.

A consequência vem junto, no `adopt_api_key()`: antes de registrar o cliente
em ninguém, o servidor lê a chave que o app tem e passa a usá-la na volta
inteira. Sem isso, "não sobrescrever" viraria o *arr registrado com uma chave
que o app não conhece — pior do que o problema que se queria evitar. App sem
chave nenhuma mantém a nossa, que é a que o `patch.rs` acabou de escrever.

A chave também vai no corpo do `setPreferences`, e o que ela faz ali é nada: medido no 5.2.3,
o `setPreferences` aceita o `web_ui_api_key`, responde 200 e **não muda o
valor** — a propriedade é espelho de leitura do `WebUI\APIKey` da conf, que é
onde ela se escreve de verdade (o `patch.rs`), e endpoint para criá-la não
existe (`apiKeys`, `generateApiKey` e afins dão 404). Por isso o servidor lê as
preferências de volta e **confere**: chave igual à do Ambiente vira linha só no
`-v`; diferente, ou vazia, vira aviso no log do trabalho — é por ela que os
*arr falam com ele, e um cliente registrado com chave que não abre nada falha
depois, longe daqui. Conferir não é falhar: quem manda na chave é a conf.

**Quem autentica é a API key**, não a senha, e quem decide isso é o
`qbit_auth()`: com uma chave no formato dele, ela é sondada no `app/version` e,
passando, vale para a volta inteira, no cabeçalho `Authorization: Bearer`. A
razão não é elegância — o mesmo `setPreferences` **troca a senha da interface**
(o `web_ui_username`/`web_ui_password` que o `qbitPrefs()` manda), e entrar com
a senha que se está prestes a substituir é uma volta que funciona uma vez e
falha na seguinte. O `auth/login` fica como reserva, para o app que não conhece
a chave — versão anterior à 5.2, ou conf que nunca recebeu uma —, e é por isso
que o `adopt_api_key()` só lê a chave do app nesse ramo: se a nossa abriu o app,
ela já é a dele.

Duas coisas da API: o `setPreferences` recebe **formulário** com um campo
`json` (não um corpo JSON), e a sessão do `auth/login`, quando é ela que vale,
vem num cookie cujo nome muda com a porta (`QBT_SID_8181`) — por isso o que se
guarda é o par inteiro, como veio. E o `has_work()` passou a contar cliente com
`prefs`: uma stack de qBittorrent sem *arr nenhum tem trabalho, e antes ela
passava em branco.

No Prowlarr, o Settings → Download Clients recebe **um registro por cliente**,
todos na categoria `CAT_PROWLARR` (`prowlarr`): o que ele pega é avulso, não veio
de instância nenhuma, então fica junto e separado do que cada *arr baixa. O campo
ali é `category`, e o nome do registro é o do cliente — é por ele que o reaplicar
acha o que já está lá. E toda categoria precisa existir *dentro* do cliente, e nos
dois casos isso é feito **pela API do app**: no qBittorrent pelo
`torrents/createCategory` (corpo de formulário, `category=…&savePath=…`; quem
já existe volta **409**, e aí é o `editCategory` com o mesmo corpo, para
reaplicar acertar a pasta em vez de falhar), e no SABnzbd pelo
`mode=set_config&section=categories`. Nenhuma é removida: pode haver torrent
apontado para ela.

O `categories.json` continua **saindo no `.zip`** — é a saída de quem não tem
servidor, e ali não há API a chamar —, mas o servidor não o escreve mais: a
entrada dele no `conf` traz `viaApi`, que é o que o `outPatches()` pula. Essa é
a regra: arquivo do app que tenha endpoint equivalente vai por API, porque
escrever o arquivo exige **parar o container**, e parar o cliente de download no
meio do Aplicar é o que fazia os *arr testarem a conexão contra um app que
estava reiniciando. Quem chama é o **Subir**, sozinho, depois de
gravar as chaves dos `patch` — e **nada é configurado antes de a inicialização
de cada app terminar**, que não é a mesma coisa que ele atender na porta.

Antes de esperar por HTTP, porém, vem uma pergunta mais barata: **o container
está no ar?** O `deploy::running()` devolve o que o `compose ps` diz, e serviço
que não está lá não é esperado — vira uma linha dizendo isso e a volta segue.
Sem isso, um container que não subiu (nome tomado por outra stack, porta já
ocupada) custava 90s de "inicializando ainda" sobre algo que nunca começou. A
lista vazia significa "não sei" — `compose ps` que falhou —, e aí a espera é a
de sempre.

A espera do `wait_apps()` tem dois passos, e a diferença entre eles é o que
fazia o primeiro Subir falhar onde o segundo passava. O `/ping` não pede chave e
é o primeiro caminho que o app serve: diz que o processo está escutando. O
`system/status`, com a chave, é o que diz que a **inicialização acabou** — banco
migrado, configuração lida, API key no lugar. Entre um e outro o *arr responde
503, ou responde 401 porque ainda não leu a própria chave, e cliente registrado
nessa janela volta como erro de validação sobre um app que só estava subindo.
Só o 200 conta como pronto; estourar as tentativas (45 × 2s) não é falha — a
volta segue, e o que ainda estiver subindo aparece no log linha a linha.

Os clientes de download também são esperados, e por um motivo próprio: escrever
a conf do qBittorrent **reinicia** o container dele, logo antes de os *arr o
testarem. Neles a pergunta é outra — a raiz do qBittorrent devolve a tela de
login enquanto ele ainda lê a conf, então quem responde por "pronto" é a API
dele (`app/version`; no SABnzbd, `api?mode=version`), e ali qualquer resposta
própria vale, 401 e 403 inclusive: o que se pergunta é se ele inicializou, não
se podemos entrar. Em todos os casos, 5xx conta como "ainda não" — é o nginx
dizendo que o de trás não subiu, e tomar isso por pronto é o mesmo que não
esperar.

O qBittorrent é registrado pela **API key**, não pela senha da interface: ela é
a mesma que a conf dele recebe, não expira quando a senha muda e é o que o campo
`apiKey` do schema espera — conferido nos dois lados: o schema do Sonarr traz
mesmo um campo `apiKey` no `QBittorrent` (ao lado de `username`/`password`), e
com só ele preenchido o teste de conexão passa. Do lado do app, o 5.2.3 lê a
chave do cabeçalho **`Authorization: Bearer`** (`webapplication.cpp`) e só a
considera se ela passar no `Utils::APIKey::isValid()`: prefixo `qbt_` e **32
caracteres no total**. Uma chave fora disso é **descartada em silêncio** na
subida — o app fica sem chave nenhuma, a autenticação por ela nunca entra, e o
*arr leva 403 sem que nada diga por quê. É o que o `api_key_valid()` do
`apply.rs` recusa antes de mandar, e o que o `qbitKeyFrom()` da página já
produz. Usuário e senha só vão para o app cujo schema não tem
esse campo — versão antiga —, e quem decide isso é o próprio schema.

O corpo de cada `downloadclient` nasce do **schema que o app publica**
(`/downloadclient/schema`), com os nossos valores por cima: mandar só os nossos
deixa o resto nulo, e o app estoura ao testar a conexão. O do Prowlarr leva
ainda um `categories: []` — é uma propriedade que os *arr não têm, e ausente ela
vira nula dentro do `ValidateCategories` dele, com um
`NullReferenceException` que não diz nada sobre a causa. Os apps são alcançados
pelo nginx, porque o servidor roda no host e a rede `starrnet` não existe para
ele; aplicar de novo procura pelo nome e atualiza no lugar, e um app fora do ar
vira uma linha no log em vez de derrubar a volta inteira.

**Chamada que não chega se repete**, no `retry()`: dez vezes, cinco segundos
entre elas. E só o que é "não consegui acessar" — erro de transporte e resposta
**5xx**, que atrás do nginx é ele dizendo que o container ainda não subiu. Erro
do app (400, 401, 404, a validação recusando o corpo) **não** se repete: a
resposta seria a mesma dez vezes, e cinquenta segundos por chamada numa volta de
dezenas delas transformaria erro de configuração em espera sem fim. Isso não
substitui o `wait_apps()` — aquele é a espera única, antes de começar; o
`retry()` é a rede para o que cai **no meio**: o qBittorrent reiniciando ao
receber a conf, o *arr ocupado importando. As tentativas saem no `-v`
(`tentativa 3/10`), e o custo do pior caso é somável: app que nunca responde
gasta os 90s da espera **mais** 50s por chamada.

A requisição é montada pela função a cada tentativa, e não clonada: corpo
consumido não se reaproveita, e o `try_clone()` do reqwest devolve `None`
justamente quando há corpo. Ao acrescentar chamada nova, monte-a dentro do
`retry()` em vez de chamar `.send()` direto. Três coisas para não
reaprender: o que vai *dentro* da aplicação do Prowlarr é o endereço interno, de
container para container, e com a base URL junto — sem ela a API do *arr fica na
raiz, onde não existe; `naming` e `mediamanagement` são recursos únicos e cheios
de campo que a página não mostra, então são lidos, mexidos nas chaves do
`naming_map()`/`MEDIA_MANAGEMENT` e devolvidos inteiros, nunca montados do zero;
e as opções de lista viajam pelo nome e chegam como número, pela ordem do
`COLON`/`MULTI_EP`, que é a mesma da página — nome fora da lista é erro, não
zero), `patch.rs` (escreve chaves na configuração que o próprio app cria:
espera o arquivo aparecer, **para** o container, faz o merge no INI e sobe de
novo — parar é o que impede o app de sobrescrever o que gravamos, porque ele
despeja a configuração em memória no disco justamente ao sair, e por isso
também o arquivo é lido *depois* do stop. O merge só troca as chaves que
vieram; comentário, ordem e o que o app guardou ficam), `jobs.rs` (trabalhos numerados com log incremental, em memória
— subir a stack baixa imagem e não cabe numa resposta HTTP), `shots.rs` (cache
em disco das capturas de paleta do theme.park, ao lado do banco, servido em
`api/shot/:app/:theme` — o `ok_segment()` recusa segmento que escaparia da pasta ou
do domínio, e o repositório continua sem redistribuir captura de ninguém: a
primeira visita sai para a documentação deles. Aberta do disco, a página busca
lá direto, como sempre).

O modelo é **normalizado**, uma tabela por conceito do estado da página:
`stack_env` (o `DEFAULTS`, uma coluna por chave, mapeadas em `ENV_COLS`, numa
linha só — o `CHECK (id = 1)` é o que a mantém única), `instance` +
`instance_lib` (o `added`), e `cfg_app`, `cfg_client`, `cfg_client_arr`,
`cfg_mm`, `cfg_naming` (o `CONFIG`). O que pende de outra tabela vai com
`ON DELETE CASCADE`. Três coisas a respeitar:

- **A chave da instância é o `cname()`** — o `container_name`. Editar o título
  muda a chave, então o `PUT` carrega o `old` e o editar vira um renomear.
- **O `PUT /api/settings` é o único caminho que apaga instância sem ninguém ter
  clicado em "Excluir"**: manda a lista de chaves, e o `reconcile()` tira o que
  não veio nela. Página com a lista errada — a que não conseguiu ler o estado,
  uma aba velha voltando à vida — apaga a stack por aí. Por isso cada PUT deixa
  uma linha na saída do servidor, com a hora, quantas chaves vieram e **quais
  saíram**: quando isso acontecer de novo, o log diz quem mandou o quê em vez de
  sobrar especulação. O `reconcile()` devolve o que apagou justamente para essa
  linha.
- **O "Excluir" apaga a pasta de configuração da instância e remove o container
  dela**, e é o **único** caminho que faz isso. O `DELETE /api/instance/:key` leva no corpo o `dir` que
  a página montou (o `cfgReal()`, o mesmo que a etiqueta da linha mostra), e o
  `remove_config_dir()` do `files.rs` **não confia nele**: exige caminho
  absoluto sem `..`, pai igual ao `BASE_CONFIG` do Ambiente e nome igual à
  chave. Qualquer uma falhando é recusa, e a recusa deixa a linha apagada e a
  pasta de pé — com um aviso no log e no corpo da resposta, nunca uma remoção
  em outro lugar. Pasta que não existe é `Ok(None)`: a instância pode nunca ter
  subido. O `reconcile()` **não** apaga pasta nenhuma, de propósito: ele tira
  linha que ninguém clicou (uma aba velha voltando à vida), e linha volta com
  um save enquanto o banco do Sonarr não volta. Na página o botão arma no
  primeiro clique — o rótulo do segundo diz o que vai junto.

  O container sai pelo `remove_container()` do `deploy.rs`, e **não** é
  `compose rm`: quando isso roda, o serviço já pode ter saído do
  `docker-compose.yml`, e o compose só remove o que o arquivo dele ainda lista.
  O `docker rm -f <nome>` alcança nos dois casos — o nome do container *é* a
  chave. E é justamente por isso que o dono é conferido antes: nome de container
  é global no daemon, e o `sonarr` da máquina pode ser de outro compose (é o
  marco v0.6 inteiro). Então o
  `com.docker.compose.project.working_dir` é lido de volta e só se remove o que
  diz ter vindo **desta** pasta; container sem etiqueta, de outra pasta ou feito
  à mão fica de pé, com o motivo no log. Os dois lados canonizam o caminho antes
  de comparar — o `--dir` pode ter chegado relativo ou por link simbólico, e
  comparar as strings como vieram deixaria o container para trás calado.

  O container vem **antes** da pasta por duas razões. É ele que escreve nela, e
  apagar a configuração debaixo de um app no ar o faz recriar metade dela na
  saída; e ele é a **porteira**: recusa ali cancela a remoção inteira, pasta
  inclusive. A razão é que recusar significa "este container não é nosso", e as
  duas metades descrevem um app só — se aquele é o `sonarr` de outra stack, a
  pasta para onde o nosso Ambiente aponta é um palpite em que não se deve
  mexer. Container que simplesmente não existe **não** é recusa: a instância
  pode nunca ter subido, e aí a pasta vai do mesmo jeito.
- **`cfg_mm` é por `service_id`**, não por instância: Media Management é por
  família, como na página.
- **`instance.extra`** guarda o que não virou coluna e volta espalhado no
  objeto. Uma flag nova no `SERVICES` não exige migração — só acrescente à
  `COLUMNS` o que precisar de coluna de verdade.
- **Chave nova no Ambiente é coluna nova, e o `schema.sql` sozinho não a
  acrescenta**: o `CREATE TABLE IF NOT EXISTS` não mexe em tabela que já
  existe. Quem a põe no banco de quem já tinha stack é o `ensure_env_cols()`,
  que roda na abertura, compara o `ENV_COLS` com o `PRAGMA table_info` e faz o
  `ALTER TABLE` do que faltar. Não é zelo: o `SELECT` do `env()` nomeia todas
  as colunas, e uma faltando derruba **toda** leitura do Ambiente — a página
  entende isso como banco vazio e o primeiro save apaga as instâncias. Já
  aconteceu, com o `jf_user`/`jf_pass`.

Tirar um serviço do catálogo não tira a instância dele do banco de quem já a
tinha. Por isso o `applyState()` filtra o que voltou
pelo `svc(id)`: sem isso, a página morre no primeiro render, procurando a cor de
um serviço que não existe, e a lista some inteira. A linha sai da interface e,
no primeiro `saveSettings()`, sai do banco junto.

O `api/health` devolve a **versão do binário** (o `CARGO_PKG_VERSION`), e é ela
que o badge `servidor v0.4.5` do cabeçalho mostra — junto da linha de subida,
que também a traz. Quem responde é o binário, não o arquivo, e os dois se
separam: ver a versão ali é o jeito mais rápido de descobrir que o servidor no
ar foi compilado antes da mudança que "não pegou". Quem pinta o badge é o
`paintServer()`, num lugar só, porque três coisas o alimentam — a versão, a
pasta da stack e se o `docker compose` respondeu — e o `applyI18n()` o chama,
para ele acompanhar o idioma. Servidor velho demais para anunciar versão
aparece sem ela, não com um `vundefined`.

O `api/health` também devolve o `puid`/`pgid` do processo — lidos do dono de
`/proc/self`, sem crate a mais. São eles o padrão de fábrica do PUID/PGID do
Ambiente quando há servidor: é ele quem cria as pastas da stack, e o app precisa
ser o mesmo dono para escrever nelas. O `detectServer()` os aplica **antes** do
`openStack()`, então o que estiver guardado no banco continua mandando.

`load()` remonta `{added, defaults, config}` na forma exata que a página espera,
e devolve `None` quando o banco ainda não tem nada guardado — assim a página
fica com os próprios padrões em vez de recebê-los em branco de volta. Esse ida e
volta sem perda é o critério do modelo; ao mexer nele, é o que os testes cobrem.

O botão **Aplicar na stack** só aparece com a stack **no ar**: ele fala com os
apps pelo nginx, e com tudo parado a volta seria dezenas de chamadas para
ninguém. Quem responde por isso é o `stackOnline()` da página, sobre o mesmo
`STATUS` do ponto da lista — daí o `paintApply()` ser chamado no
`paintStatus()`, e não só no `openCfg()`: stack que sobe (ou cai) com o modal
aberto muda o botão junto com os pontos.

O modal do log — o **Subindo a stack**, o **Derrubando a stack** e o **Aplicando
a Configuração** — trava o Fechar enquanto o trabalho corre: fechá-lo não
cancelaria nada, porque quem roda o `docker compose` e as chamadas de API é o
servidor, e sumiria com o único lugar em que dá para acompanhar. O `runJob()` o
libera quando o trabalho termina, tendo dado certo ou não — inclusive quando ele
nem começa, que é o caso do servidor fora do ar.

Quem quer sair antes do fim tem o **Parar**, ao lado do Fechar: ele chama
`POST /api/job/:id/stop`, e ali o `jobs.rs` **aborta a tarefa de dentro** — a
mesma que o pânico já matava —, de modo que o trabalho termina como falha
comum, com `done` escrito e o Fechar de volta. Não é o clique que libera o
modal, é o fim do trabalho: assim a tela nunca fica adiantada em relação ao
servidor. O `docker compose` que estiver rodando morre junto pelo
`kill_on_drop(true)` do `deploy.rs` — sem ele o processo continuaria sozinho,
sem ninguém lendo a saída. Parar deixa a stack no meio do caminho (containers
meio subidos, Configuração meio aplicada), e é isso que a `log.stopped` diz e o
`record()` do servidor guarda.

**Toda saída daquele laço tem de passar pelo `endLog()`.** Enquanto ele gira, o
Fechar está desabilitado, então um caminho que não termine prende o modal para
sempre — com a tela parada, que é o pior jeito de falhar. Foram dois buracos
assim, um de cada lado:

- na página, a busca do trabalho que falha (servidor que caiu, trabalho que ele
  não conhece mais — eles vivem em memória) contava como "ainda correndo"; hoje
  ela conta as falhas seguidas e desiste depois de `ATTEMPTS`, com a
  `log.lost` no log;
- no servidor, pânico dentro do trabalho matava a tarefa antes do `done`, e a
  página perguntava por ele para sempre. O `jobs.rs` roda o trabalho numa
  tarefa de dentro e espera pelo `JoinHandle`, o que transforma o pânico numa
  falha comum.

Do lado da página, a seção `/* ---------- servidor ---------- */`:
`detectServer()` só faz algo em `http(s)://` e chama `openStack()`, que carrega
o estado guardado — sem id nenhum, porque a stack é a do servidor.
`putInstance`/`delInstance` mexem numa linha por vez, e `saveSettings()`
(debounce no fim do `render()`) manda Ambiente, Configuração e a lista de chaves — é ela que acerta a ordem e apaga o
que saiu sem passar pelo modal. A flag `loading` existe para o estado que vem do
banco não ser gravado de volta enquanto está sendo aplicado.

A outra flag, a `readOnly`, é a rede de segurança dessa mesma lista de chaves:
carregamento que **falhou** (qualquer resposta do `api/state` que não seja 200
ou 204) deixa a tela com uma stack vazia que não é a do banco, e seguir daí
grava essa lista por cima — o `reconcile()` apaga o que não vier nela. Então o
`openStack()` a liga, o aviso `#noState` aparece e as três funções que gravam
(`putInstance`, `delInstance`, `saveSettings`) desistem até alguém recarregar a
página. 204 é banco vazio e continua sendo começo normal.
