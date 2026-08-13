#!/usr/bin/env python3
"""Abre a página num navegador sem tela, monta uma stack e passa o que ela
gerou pelo `docker compose config`.

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
  ta.textContent = JSON.stringify(outFiles());
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


def main():
    pagina = sys.argv[1] if len(sys.argv) > 1 else 'hubstarr.html'
    # a pasta vai no HOME, não em /tmp: o chromium empacotado como snap não lê
    # /tmp, e a página abriria em branco — o erro que aparece é a página não
    # ter montado stack nenhuma, que não diz nada sobre a causa
    with tempfile.TemporaryDirectory(dir=os.path.expanduser('~')) as saida:
        arquivos = gerar(pagina, saida)
        nomes = [f['name'] for f in arquivos]
        print('a página gerou:', ', '.join(nomes))

        stack = os.path.join(saida, 'stack')
        os.makedirs(stack)
        for f in arquivos:
            if f['name'] in ('docker-compose.yml', '.env'):
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


if __name__ == '__main__':
    main()
