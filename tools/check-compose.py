#!/usr/bin/env python3
"""Abre a página num navegador sem tela, monta uma stack e passa o que ela
gerou pelo `docker compose config` e o `nginx.conf` pelo `nginx -t`.

É a checagem que mais paga: os geradores emitem HTML com spans de realce, e o
texto que vai para o arquivo sai do `textContent` dos panes. Um `${...}` mal
fechado, uma indentação trocada ou uma chave a mais no lugar errado só
aparecem quando o docker recusa o arquivo — na máquina de quem usa, se ninguém
tiver olhado antes.

A stack de exemplo tem um pouco de cada coisa que muda a forma do compose:
duas instâncias da mesma família, roteamento pela VPN (que traz o gluetun e o
`network_mode`), serviço que publica porta em vez de virar rota, GPU, e um
serviço `internal`.
"""
import html
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

STACK = """
DEFAULTS.cfg  = '/tmp/hubstarr-ci/config';
DEFAULTS.data = '/tmp/hubstarr-ci/media';
DEFAULTS.dl   = '/tmp/hubstarr-ci/downloads';
DEFAULTS.apiKey = '0123456789abcdef0123456789abcdef';
added = [
  {id:'sonarr',      title:'Sonarr',     data:'tv'},
  {id:'sonarr',      title:'Sonarr 4K',  data:'tv-4k', tpv:'uhd'},
  {id:'radarr',      title:'Radarr',     data:'movies'},
  {id:'prowlarr',    title:'Prowlarr'},
  {id:'qbittorrent', title:'qBittorrent', data:'torrents', vpn:true},
  {id:'sabnzbd',     title:'SABnzbd',    data:'usenet'},
  {id:'jellyfin',    title:'Jellyfin',   hw:'intel'},
  {id:'seerr',       title:'Seerr'},
  {id:'flaresolverr',title:'FlareSolverr'},
];
"""


# O que o navegador precisa para rodar sem sessão gráfica e sair sozinho. O
# `--user-data-dir` próprio é o que evita ele travar quando já há um perfil em
# uso, e o `--virtual-time-budget` é o que encerra a página em vez de esperar
# um temporizador que nunca vem.
OPCOES = [
    '--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage',
    '--no-first-run', '--no-default-browser-check',
    '--disable-extensions', '--disable-background-networking', '--disable-sync',
    '--virtual-time-budget=8000',
]


def abrir(navegador, pagina, saida):
    """O DOM da página depois de ela rodar.

    Duas tentativas, com o headless novo e com o antigo: o novo é o que vale
    daqui para a frente, mas em runner de CI ele às vezes não devolve o DOM e
    fica pendurado até o timeout — e aí o antigo resolve. Sem a segunda
    tentativa, a checagem falharia por causa do navegador, não da página.
    """
    erro = None
    for modo in ('--headless=new', '--headless=old'):
        try:
            r = subprocess.run(
                [navegador, modo, *OPCOES, f'--user-data-dir={saida}/perfil-{modo[-3:]}',
                 '--dump-dom', f'file://{pagina}'],
                capture_output=True, text=True, timeout=60)
            if r.stdout.strip():
                return r.stdout
            erro = f'{modo}: saiu sem DOM ({r.stderr.strip()[:200]})'
        except subprocess.TimeoutExpired:
            erro = f'{modo}: não respondeu em 60s'
        print(f'  {erro}, tentando de novo', file=sys.stderr)
    sys.exit(f'o navegador não abriu a página — {erro}')


def gerar(pagina, saida):
    """Roda a página no chromium e devolve os arquivos que ela gera."""
    injecao = """
try{
  %s
  applyI18n(); renderCombo(); renderItems(); render();
  const ta = document.createElement('textarea');
  ta.id = 'saida';
  SERVER = {ok:true, docker:true, dir:'/srv/stack'};
  ta.textContent = JSON.stringify({arquivos: outFiles(),
                                   arrs: applyPayload().arrs,
                                   jellyfin: applyPayload().jellyfin});
  document.body.appendChild(ta);
  document.title = 'OK';
}catch(e){ document.title = 'ERRO: ' + e.message + ' @@ ' + (e.stack||'').split('\\n')[1]; }
""" % STACK

    corpo = open(pagina, encoding='utf-8').read()
    # a injeção entra no fim do script; o `startTour()` fica de fora, senão a
    # volta da primeira visita cobre a página
    alvo = 'detectServer();\nstartTour();\n</script>'
    if alvo not in corpo:
        sys.exit('não achei o fim do <script> da página')
    corpo = corpo.replace(alvo, 'detectServer();\n' + injecao + '\n</script>', 1)

    tmp = os.path.join(saida, 'pagina.html')
    open(tmp, 'w', encoding='utf-8').write(corpo)

    # o `$CHROME` manda, quando alguém o aponta; fora isso, o primeiro que
    # existir na máquina
    navegador = os.environ.get('CHROME') or next(
        (n for n in ('google-chrome', 'chromium', 'chromium-browser', 'chrome')
         if shutil.which(n)), None)
    if not navegador:
        sys.exit('sem um navegador para abrir a página (instale o chromium ou aponte $CHROME)')
    print('navegador:', navegador)

    dom = abrir(navegador, tmp, saida)

    titulo = html.unescape(re.search(r'<title>(.*?)</title>', dom, re.S).group(1))
    if not titulo.startswith('OK'):
        sys.exit(f'a página não montou a stack — {titulo}')

    bruto = re.search(r'<textarea id="saida">(.*?)</textarea>', dom, re.S)
    return json.loads(html.unescape(bruto.group(1)))


def conferir_bibliotecas(compose, jf):
    """Cada biblioteca do Jellyfin é um caminho que ele enxerga.

    Mesmo erro das pastas raiz, e mais calado ainda: o Jellyfin aceita qualquer
    caminho e a biblioteca simplesmente nasce vazia. O que ele monta é a base
    inteira em `/data` mais uma pasta por caminho de fora, e é contra esses
    binds que as bibliotecas têm de bater.
    """
    if not jf:
        return
    import yaml
    conf = yaml.safe_load(compose)
    alvos = {v['target'] for v in conf['services']['jellyfin'].get('volumes', [])
             if isinstance(v, dict) and v['target'].startswith('/data')}
    ruim = 0
    for lib in jf['libs']:
        # a base cobre tudo o que está debaixo dela
        if lib['path'] in alvos or ('/data' in alvos and lib['path'].startswith('/data/')):
            continue
        print(f"jellyfin: biblioteca {lib['path']} não é montada "
              f"(o compose monta {sorted(alvos)})", file=sys.stderr)
        ruim += 1
    if ruim:
        sys.exit('biblioteca que o container não enxerga')
    print(f"{len(jf['libs'])} bibliotecas do Jellyfin batem com os binds")


def conferir_nginx(stack):
    """`nginx -t` contra o `nginx.conf` que a página gerou.

    O texto sai do `textContent` do pane, sem nunca passar por um nginx de
    verdade — um `location` mal fechado ou um `$upstream` sem `resolver` só
    apareciam depois, na máquina de quem sobe a stack. Roda com a mesma
    imagem do compose (`nginx:alpine`) e o mesmo alvo do bind
    (`/etc/nginx/conf.d/nginx.conf`), para testar exatamente o arquivo que o
    container vai ler — `default.conf`, que vem na imagem, entra na mesma
    checagem.
    """
    alvo = os.path.join(stack, 'nginx.conf')
    r = subprocess.run(
        ['docker', 'run', '--rm',
         '-v', f'{alvo}:/etc/nginx/conf.d/nginx.conf:ro',
         'nginx:alpine', 'nginx', '-t'],
        capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stderr, file=sys.stderr)
        sys.exit('o nginx recusou o nginx.conf gerado')
    print('nginx.conf válido')


def conferir_raizes(compose, arrs):
    """A pasta raiz de cada *arr é o caminho que o container enxerga.

    O app guarda ali o que baixa, e o caminho que ele recebe tem de ser o
    destino de um bind — não o do host. Mandar o do host não dá erro nenhum na
    hora: o *arr aceita, e só depois não acha arquivo nenhum. Os dois lados
    saem da mesma página, e é justamente por isso que podem divergir sem que
    ninguém veja.
    """
    import yaml
    conf = yaml.safe_load(compose)
    ruim = 0
    for arr in arrs:
        servico = conf['services'].get(arr['key'], {})
        alvos = {v['target'] for v in servico.get('volumes', [])
                 if isinstance(v, dict) and v['target'].startswith('/data')}
        for pasta in arr.get('rootFolders', []):
            if pasta not in alvos:
                print(f"{arr['key']}: pasta raiz {pasta} não é montada "
                      f"(o compose monta {sorted(alvos) or 'nada em /data'})", file=sys.stderr)
                ruim += 1
    if ruim:
        sys.exit('pasta raiz que o container não enxerga')
    print('pastas raiz batem com os binds')


def main():
    pagina = sys.argv[1] if len(sys.argv) > 1 else 'hubstarr.html'
    # a pasta vai no HOME, não em /tmp: o chromium empacotado como snap não lê
    # /tmp, e a página abriria em branco — o erro que aparece é a página não
    # ter montado stack nenhuma, que não diz nada sobre a causa
    with tempfile.TemporaryDirectory(dir=os.path.expanduser('~')) as saida:
        saiu = gerar(pagina, saida)
        arquivos = saiu['arquivos']
        nomes = [f['name'] for f in arquivos]
        print('a página gerou:', ', '.join(nomes))

        stack = os.path.join(saida, 'stack')
        os.makedirs(stack)
        for f in arquivos:
            if f['name'] in ('docker-compose.yml', '.env', 'nginx.conf'):
                open(os.path.join(stack, f['name']), 'w', encoding='utf-8').write(f['text'])

        r = subprocess.run(['docker', 'compose', 'config'],
                           cwd=stack, capture_output=True, text=True)
        if r.returncode != 0:
            print(r.stderr, file=sys.stderr)
            sys.exit('o docker recusou o compose gerado')

        # o que o compose entendeu tem de ser o que a página anunciou
        servicos = subprocess.run(['docker', 'compose', 'config', '--services'],
                                  cwd=stack, capture_output=True, text=True).stdout.split()
        print('serviços no compose:', ', '.join(sorted(servicos)))
        for esperado in ('nginx', 'gluetun', 'sonarr', 'sonarr-4k'):
            if esperado not in servicos:
                sys.exit(f'o compose saiu sem o {esperado}')
        print('compose válido')

        conferir_nginx(stack)

        compose = [f for f in arquivos if f['name'] == 'docker-compose.yml'][0]['text']
        conferir_raizes(compose, saiu['arrs'])
        conferir_bibliotecas(compose, saiu.get('jellyfin'))


if __name__ == '__main__':
    main()
