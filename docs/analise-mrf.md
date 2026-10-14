# Análise do formato `.mrf` (Macro Recorder 4.x)

Notas de engenharia reversa **funcional**: o que o arquivo guarda e como o programa
o usa. Nenhum código ou recurso do original foi copiado. O Korvon Macro usa o
próprio formato (`.kmacro`, JSON); isto serve para mapear conceitos e, no futuro,
escrever um importador.

Fonte: decompilação do executável no Ghidra + tabelas de strings/formulários.
Confiança: **validada** — o layout foi conferido contra um `.mrf` real (483 ações, leitura termina exatamente no fim do arquivo) para a versão 4 000 043; **alta** para os limiares de versão; **média** para
a ordem exata dos 6 filtros booleanos do cabeçalho; **pendente** para o significado
dos `Param1..5` em cada tipo de ação.

## Contêiner

Fluxo binário little-endian, sem compressão aparente. Strings são `u32` com o
tamanho **em bytes** seguido de UTF-16LE (sem terminador).

| Parte | Tamanho | Conteúdo |
|---|---|---|
| Versão | 8 B | dois `u32` iguais; versão atual `4 000 043` (0x3D092B). A versão lida (`V`) condiciona campos opcionais |
| Info de tela | 20 B | `u32` nº de monitores + 4 `u32` (retângulo da área de trabalho) |
| Monitores | 12 B × n | por monitor: largura, altura, escala (%) — usados no aviso de "resolução mudou" |
| Cabeçalho | 48 B (V atual) | ver abaixo; versões antigas usam 24/28 B |
| Ações | variável | `count` registros de ação, em sequência |

Cabeçalho de 48 B (V atual): `u32` nº de ações · `u32` (campo de velocidade/repetição)
· 6 bytes de filtros de exibição (mouse, cliques, teclas, esperas, foco, notificações)
· 1 byte (mouse relativo) · `u32` modo de caminho do mouse · `u32` · 1 byte (salvar
caminho) · 22 bytes (data de criação em formato interno do Delphi).

## Registro de ação

Ordem de gravação (campo → tipo → versão mínima `V >`):

| # | Campo | Tipo | Versão |
|---|---|---|---|
| 1 | ActionType | u32 (ordinal, tabela abaixo) | — |
| 2 | Delay (ms) | u32 | — |
| 3–7 | Param1..Param5 | u32 × 5 | — |
| 8 | Count | u32 | — |
| 9 | ExtraData | u32 | 1 000 051 |
| 10 | Enabled | u8 | 3 000 000 |
| 11 | GroupColor | u32 | 3 000 000 |
| 12 | EndOfGroup | u8 | 3 000 007 |
| 13 | Expanded | u8 | 3 000 007 |
| 14 | IsBreakpoint | u8 | 3 999 999 |
| 15 | Text | string | — |
| 16 | Text2 | string | 3 998 997 |
| 17 | LabelName | string | — |
| 18 | TargetLabel | string | 1 000 051 |
| 19 | TimeoutLabel | string | 1 000 051 |
| 20 | Comment | string | 2 000 010 |
| 21 | Group (nome) | string | 3 000 000 |
| 22 | nº de subações "verdadeiro" + registros | u32 + ações | 3 000 000 |
| 23 | nº de subações "falso" + registros | u32 + ações | 3 000 000 |

Subações são registros de ação aninhados do mesmo formato (usadas por condições e
grupos). A leitura aborta o arquivo se um registro falhar.

## Parâmetros observados em um arquivo real

Cabeçalho do exemplo: 2 monitores 1920×1080 (escala 1.0), área de trabalho 0,0–3840×1080,
filtros de exibição todos ligados, velocidade `100` (%).

| Tipo | Observação |
|---|---|
| 8 Troca de janela | `Text` = `executável|título (com curingas)|`; `Param1..4` = x, y, largura, altura da janela (-1 em `Param5`); `Count` = 2 |
| 0 Mover mouse | `Param1,2` = x,y inicial; `Param3,4` = x,y final; `Delay` = duração do movimento (ms) |
| 3 Clique | `Param1` = botão (1 = esquerdo); `Param3,4` = x,y do clique |
| 7 Esperar | `Delay` e `Param1` = tempo (ms) |
| 6 Caracteres | `Text` = caracteres digitados (maiúsculas, sem Shift); `Param1`/`Param2` = código de tecla virtual do primeiro/último caractere |
| 4 Tecla (down/up) | `Param1` = código de tecla virtual; `Param2` = scancode; `Param3` = 1 pressionar / 0 soltar; `Param4` = 1 se tecla estendida |
| 5 Tecla pressionada | mesmos parâmetros do tipo 4 (`Param3` = 1) — pressionar e soltar |
| 3 Clique (completo) | `Param1` = botão (1 esquerdo, 2 direito); `Param2` = 1 (clique simples) |

## Tipos de ação (ordinal → nome)

0 Mouse move · 1 Botão do mouse (down/up) · 2 Roda horizontal/vertical · 3 Duplo clique ·
4 Tecla (down/up) · 5 Tecla pressionada · 6 Caracteres · 7 Esperar · 8 Troca de janela ·
9 Esperar cor de pixel · 10 Saída de texto · 11 Ir para rótulo · 12 Repetir a partir de
rótulo · 14 Esperar hotkey · 15 Esperar texto digitado · 16 Executar programa ·
17 Buscar imagem · 18 SmartClick · 19 Esperar arquivo · 20 Buscar texto (OCR) ·
21 Randomizar parâmetro · 22 Incrementar parâmetro · 23 Parâmetro de lista ·
24 Definir variável · 25 Valor a partir de variável · 26 Condição · 27 Capturar texto ·
28 Capturar imagem · 29 Código de barras/QR · 30 Notificação · 31 Caixa de mensagem ·
32 Embutir macro · 33 IA texto · 34 Beep · 35 Calcular · 36 Executar frase ·
37 IA analisar imagem · 38 IA gerar imagem · 39 Salvar variável · 40 Editar imagem ·
41 Esperar mudança na tela · 42 Extrair de site · 43 Variável a partir de lista de
dados · 44 IA busca de objeto.

(13 não aparece na função de descrição; provável tipo interno/obsoleto.)

## Configurações do programa (chaves INI observadas)

Gravação: `MouseMoveInterval`, `MouseMoveDelta`, `MouseUpDownSleep`, `KeysAsChars`,
`CharsAsText`, `NoModifierDownCount`, `CleanUp`, `DefaultSaveMousePath`,
`CountSameActions`, `UseGreyScaleForClicks`, `DefaultClickColorTolerance`.
Reprodução: `PauseOnMouse`, `PauseOnKeys`, `BlockInput`, `AlwaysUseVK`,
`RestoreMousePos`, `FixWindowRect`, `RelativeMouseMove`, `ResetState`.

## Agendamento

O original gera um XML de tarefa e chama `schtasks.exe /Create /XML ... /tn ...`
(delete: `/Delete /tn ... /F`). Gatilhos: calendário (dia/semana/mês, repetição
`PT<n>M`/`PT<n>H`) e logon. Ação: executar o programa com argumentos.

Validado com dois arquivos reais (483 e 859 ações), ambos lidos até o último byte.

Observação sobre a decompilação dos diálogos: `Goto` guarda o rótulo de destino em
`Text`; `Repeat` guarda o modo em `Param1` (1 = contagem, 2 = infinito, 3 = por tempo,
4 = até horário). O restante dos tipos de fluxo (condição, variáveis, esperas,
visão) não tem amostra real e a decompilação de interface é ruidosa.

## Pendências

1. Significado de `Param1..5`/`Count`/`Text` por tipo de ação (decompilar os
   métodos `LoadMacroAction`/`SaveMacroAction` de cada diálogo e `Param1String`).
2. Motor de reprodução (ordem de avaliação, timeouts, saltos, loops).
3. DLLs `mrkey`, `mrocr`, `mrzbar`.
4. Validar o layout com um `.mrf` real gravado pelo programa.

## Comportamento do gravador (deduzido de gravações reais)

Observado em dois arquivos gravados com as configurações padrão (≈ 1 700 registros):

- **Movimento do mouse:** um registro por amostra, com ponto inicial = ponto final do registro
  anterior e `Delay` = milissegundos desde o evento anterior (tipicamente 1–25 ms, até 481).
  O ponto inicial só difere do anterior após cliques/esperas.
- **Espera (tipo 7):** gravada somente quando o intervalo de inatividade é de **500 ms ou mais**
  (menor espera observada 509 ms; maior duração de movimento 481 ms). Intervalos menores ficam
  dentro da duração do movimento. A ação seguinte a uma espera tem `Delay` = 50.
- **Clique (tipo 3):** um único registro com o botão, a posição e `Delay` = 50 (retenção).
- **Teclado:** letras, dígitos e espaço consecutivos viram um único registro de caracteres
  (tipo 6, `Delay` = 50 por caractere). Teclas especiais viram tecla pressionada (tipo 5).
  Modificadores presos enquanto outra tecla é usada geram tecla down e up separados (tipo 4,
  `Param3` = 1/0); um modificador sozinho vira tecla pressionada (tipo 5).
