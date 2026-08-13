#!/usr/bin/env python3
"""Confere que os três idiomas do `I18N` têm exatamente as mesmas chaves.

Chave que existe em pt-BR e falta em en aparece na interface como o próprio
nome da chave — `cfg.profHint` no lugar da frase. Não quebra nada, então passa
despercebido até alguém trocar de idioma; é o tipo de coisa que uma máquina
acha melhor do que uma pessoa.

O `I18N` é um objeto literal dentro do `<script>`, e aqui ele é lido como
texto: as chaves são as que aparecem em começo de linha, com dois espaços de
indentação, dentro do bloco de cada idioma. Não é um parser de JavaScript, e
não precisa ser — o arquivo tem uma forma só, e a regra de indentação é dele.
"""
import re
import sys

# a linha que abre cada idioma: `'pt-BR':{`
IDIOMA = re.compile(r"^'([\w-]+)':\{$")
# uma chave de tradução: dois espaços, o nome entre aspas, dois-pontos
CHAVE = re.compile(r"^  '([^']+)':")


def blocos(script):
    """{idioma: [chaves]}, na ordem em que aparecem no arquivo."""
    dentro, achados = None, {}
    for linha in script.splitlines():
        m = IDIOMA.match(linha)
        if m:
            dentro = m.group(1)
            achados[dentro] = []
            continue
        if dentro is None:
            continue
        # `};` na margem fecha o bloco do idioma
        if linha.startswith('}'):
            dentro = None
            continue
        k = CHAVE.match(linha)
        if k:
            achados[dentro].append(k.group(1))
    return achados


def main(caminho):
    html = open(caminho, encoding='utf-8').read()
    corpo = re.findall(r'<script>(.*?)</script>', html, re.S)[-1]
    # do `const I18N = {` até o fim do objeto, para não varrer o arquivo todo
    ini = corpo.index('const I18N = {')
    idiomas = blocos(corpo[ini:])
    if len(idiomas) < 2:
        sys.exit('não achei os blocos de idioma do I18N')

    problemas = []
    for idioma, chaves in idiomas.items():
        repetidas = {k for k in chaves if chaves.count(k) > 1}
        if repetidas:
            problemas.append(f'{idioma}: chave repetida — {", ".join(sorted(repetidas))}')

    base, *outros = idiomas
    for idioma in outros:
        falta = set(idiomas[base]) - set(idiomas[idioma])
        sobra = set(idiomas[idioma]) - set(idiomas[base])
        if falta:
            problemas.append(f'{idioma}: falta {len(falta)} — {", ".join(sorted(falta))}')
        if sobra:
            problemas.append(f'{idioma}: sobra {len(sobra)} — {", ".join(sorted(sobra))}')

    if problemas:
        print('\n'.join(problemas), file=sys.stderr)
        sys.exit(1)
    print(f'{len(idiomas)} idiomas, {len(idiomas[base])} chaves cada — iguais')


if __name__ == '__main__':
    main(sys.argv[1] if len(sys.argv) > 1 else 'hubstarr.html')
