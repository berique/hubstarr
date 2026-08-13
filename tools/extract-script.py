#!/usr/bin/env python3
"""Escreve na saída o `<script>` inline da página.

A página é um arquivo só, com o script dentro dela: para passar qualquer
ferramenta de JavaScript nele, primeiro é preciso tirá-lo de lá. Vale o último
`<script>` do arquivo, que é o da aplicação — os outros, se houver, são
`type="application/json"` e coisas do gênero.
"""
import re
import sys

def script_de(caminho):
    html = open(caminho, encoding='utf-8').read()
    blocos = re.findall(r'<script>(.*?)</script>', html, re.S)
    if not blocos:
        sys.exit(f'{caminho}: não achei o <script> da página')
    return blocos[-1]

if __name__ == '__main__':
    sys.stdout.write(script_de(sys.argv[1]))
