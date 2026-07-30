import os
import requests
from requests.auth import HTTPBasicAuth
import openpyxl
from openpyxl.styles import Font, PatternFill, Alignment, Border, Side
from dotenv import load_dotenv

# Carrega as variáveis de ambiente do arquivo .env
load_dotenv()

# --- CONFIGURAÇÕES CARREGADAS DO .ENV ---
# é o e-mail
USERNAME = os.getenv("BITBUCKET_USER")
# precisa das permissões read:repository:bitbucket e read:workspace:bitbucket
APP_PASSWORD = os.getenv("BITBUCKET_APP_PASSWORD")
WORKSPACE = os.getenv("BITBUCKET_WORKSPACE")
CAMINHO_EXCEL = os.getenv("CAMINHO_EXCEL", "relatorio_de_permissoes_bitbucket.xlsx")
# ----------------------------------------

if not all([USERNAME, APP_PASSWORD, WORKSPACE]):
    raise ValueError("Erro: Verifique se as variáveis BITBUCKET_USER, BITBUCKET_APP_PASSWORD e BITBUCKET_WORKSPACE estão preenchidas no arquivo .env")

BASE_URL = "https://api.bitbucket.org/2.0/repositories"
auth = HTTPBasicAuth(USERNAME, APP_PASSWORD)

class ApiError(Exception):
    """Erro de API que não é um 404 simples (ex: 403, 429, 500) - precisa ser
    tratado como falha real e não confundido com 'sem dados'."""
    def __init__(self, status_code, text):
        self.status_code = status_code
        self.text = text
        super().__init__(f"HTTP {status_code} - {text}")

def fetch_paginated_data(url):
    """Auxiliar para lidar com paginação da API do Bitbucket.
    Retorna None para 404 (recurso inexistente/sem acesso).
    Levanta ApiError para qualquer outro erro (403, 429, 500...), para que o
    chamador não confunda 'erro' com 'lista vazia'."""
    results = []
    while url:
        response = requests.get(url, auth=auth)
        if response.status_code == 404:
            return None
        if response.status_code != 200:
            raise ApiError(response.status_code, response.text)
        data = response.json()
        results.extend(data.get('values', []))
        url = data.get('next')
    return results

def listar_todos_repositorios():
    """Busca dinamicamente todos os repositórios do Workspace que o usuário tem acesso"""
    print(f"Buscando a lista de repositórios no Workspace '{WORKSPACE}'...")
    # Endpoint filtrado pelo Workspace para trazer todos os repositórios dele
    url = f"{BASE_URL}/{WORKSPACE}"
    try:
        repos_data = fetch_paginated_data(url)
    except ApiError as e:
        print(f"[ERRO] Falha ao listar repositórios do Workspace '{WORKSPACE}': HTTP {e.status_code} - {e.text}")
        return []

    if not repos_data:
        print(f"[AVISO] Nenhum repositório retornado pela API para o Workspace '{WORKSPACE}'.")
        return []

    # Extrai o 'slug' de cada repositório junto com o Projeto ao qual pertence -
    # permissões concedidas no nível do Projeto se aplicam a todos os seus repos,
    # mesmo sem nenhuma entrada própria em permissions-config do repositório.
    repos = []
    for repo in repos_data:
        projeto = repo.get('project') or {}
        repos.append({
            'slug': repo['slug'],
            'project_key': projeto.get('key'),
            'project_name': projeto.get('name') or projeto.get('key') or '',
        })
    print(f"[OK] {len(repos)} repositórios listados pela API.")
    return repos

# Cache de permissões por Projeto: várias repos pertencem ao mesmo Projeto, então
# buscamos as permissões do Projeto uma única vez e reaproveitamos para todos eles.
_PROJECT_PERMS_CACHE = {}

def coletar_dados_projeto(project_key, project_name):
    """Busca permissões diretas de usuários e de grupos configuradas no nível do
    Projeto. Essas permissões se aplicam a TODOS os repositórios do projeto, mesmo
    que o repositório em si não tenha nenhuma entrada própria em permissions-config
    - por isso precisam ser buscadas mesmo quando o repositório está "vazio"."""
    if project_key in _PROJECT_PERMS_CACHE:
        return _PROJECT_PERMS_CACHE[project_key]

    print(f"  Buscando permissões de nível de Projeto para '{project_name}' ({project_key})...")
    linhas = []
    url_base_projeto = f"https://api.bitbucket.org/2.0/workspaces/{WORKSPACE}/projects/{project_key}"

    try:
        user_perms = fetch_paginated_data(f"{url_base_projeto}/permissions-config/users")
    except ApiError as e:
        print(f"    [ERRO] Projeto '{project_key}': falha ao buscar permissões de usuários (HTTP {e.status_code}).")
        linhas.append([f"Projeto ({project_name})", f"ERRO AO ACESSAR PROJETO (HTTP {e.status_code})", "-", "-"])
        user_perms = []

    if user_perms is None:
        print(f"    Projeto '{project_key}': nenhuma permissão direta de usuário (404).")
        user_perms = []
    elif user_perms:
        print(f"    [OK] Projeto '{project_key}': {len(user_perms)} permissão(ões) direta(s) de usuário.")
    else:
        print(f"    Projeto '{project_key}': nenhuma permissão direta de usuário.")

    for perm in user_perms:
        linhas.append([f"Projeto ({project_name})", perm['user']['display_name'], f"@{perm['user']['nickname']}", perm['permission'].upper()])

    try:
        group_perms = fetch_paginated_data(f"{url_base_projeto}/permissions-config/groups")
    except ApiError as e:
        print(f"    [ERRO] Projeto '{project_key}': falha ao buscar permissões de grupos (HTTP {e.status_code}).")
        linhas.append([f"Projeto ({project_name})", f"ERRO AO ACESSAR GRUPOS DO PROJETO (HTTP {e.status_code})", "-", "-"])
        group_perms = []

    if group_perms is None:
        print(f"    Projeto '{project_key}': nenhum grupo configurado (404).")
        group_perms = []
    elif group_perms:
        print(f"    [OK] Projeto '{project_key}': {len(group_perms)} grupo(s) com permissão.")
    else:
        print(f"    Projeto '{project_key}': nenhuma permissão de grupo.")

    for g_perm in group_perms:
        group_slug = g_perm['group']['slug']
        group_name = g_perm['group']['name']
        level = g_perm['permission'].upper()
        print(f"      Coletando membros do grupo '{group_name}' ({group_slug}) [nível Projeto] com permissão '{level}'...")

        members_url = f"https://api.bitbucket.org/2.0/workspaces/{WORKSPACE}/permissions/config/groups/{group_slug}/members"
        try:
            members = fetch_paginated_data(members_url)
        except ApiError as e:
            print(f"      [ERRO] Falha ao buscar membros do grupo '{group_name}' (HTTP {e.status_code}).")
            linhas.append([f"Projeto ({project_name}) > Grupo ({group_name})", f"ERRO AO ACESSAR MEMBROS (HTTP {e.status_code})", "-", level])
            continue

        if not members:
            print(f"      Grupo '{group_name}' não possui membros ou não foi possível acessar os membros.")
            linhas.append([f"Projeto ({project_name}) > Grupo", group_name, "(Grupo Vazio)", level])
        else:
            print(f"      [OK] {len(members)} membro(s) encontrado(s) no grupo '{group_name}'.")
            for member in members:
                linhas.append([f"Projeto ({project_name}) > Grupo ({group_name})", member['display_name'], f"@{member['nickname']}", level])

    _PROJECT_PERMS_CACHE[project_key] = linhas
    return linhas

def coletar_dados_repositorio(repo_slug, project_key=None, project_name=None):
    """Busca acessos diretos, por grupo e herdados do Projeto de um repositório."""
    dados_repo = []
    url_base_repo = f"{BASE_URL}/{WORKSPACE}/{repo_slug}"
    repo_acessivel = True

    # 1. Permissões Diretas de Usuários
    try:
        user_perms = fetch_paginated_data(f"{url_base_repo}/permissions-config/users")
    except ApiError as e:
        print(f"  [ERRO] '{repo_slug}': falha ao buscar permissões de usuários (HTTP {e.status_code}). Repositório marcado como erro, não será omitido silenciosamente.")
        dados_repo.append([repo_slug, "N/A", f"ERRO AO ACESSAR (HTTP {e.status_code})", "-", "-"])
        user_perms = None
        repo_acessivel = False

    if repo_acessivel and user_perms is None:
        print(f"  [AVISO] '{repo_slug}': 404 em permissions-config/users - sem acesso direto ou repositório inacessível.")
        dados_repo.append([repo_slug, "N/A", "SEM ACESSO OU INACESSÍVEL", "-", "-"])
        repo_acessivel = False
    elif repo_acessivel:
        if user_perms:
            print(f"  [OK] '{repo_slug}': {len(user_perms)} permissão(ões) direta(s) de usuário encontrada(s).")
        else:
            print(f"  '{repo_slug}': nenhuma permissão direta de usuário.")
        for perm in user_perms:
            dados_repo.append([
                repo_slug, "Direto", perm['user']['display_name'], f"@{perm['user']['nickname']}", perm['permission'].upper()
            ])

    # 2. Permissões de Grupos e membros (só faz sentido se o repo respondeu)
    if repo_acessivel:
        try:
            group_perms = fetch_paginated_data(f"{url_base_repo}/permissions-config/groups")
        except ApiError as e:
            print(f"  [ERRO] '{repo_slug}': falha ao buscar permissões de grupos (HTTP {e.status_code}).")
            dados_repo.append([repo_slug, "N/A", f"ERRO AO ACESSAR GRUPOS (HTTP {e.status_code})", "-", "-"])
            group_perms = []

        if group_perms is None:
            # 404 aqui normalmente significa "sem grupos configurados", não um erro.
            print(f"  '{repo_slug}': nenhum grupo configurado (404).")
            group_perms = []
        elif group_perms:
            print(f"  [OK] '{repo_slug}': {len(group_perms)} grupo(s) com permissão encontrado(s).")
        else:
            print(f"  '{repo_slug}': nenhuma permissão de grupo.")

        for g_perm in group_perms:
            group_slug = g_perm['group']['slug']
            group_name = g_perm['group']['name']
            level = g_perm['permission'].upper()
            print(f"    Coletando membros do grupo '{group_name}' ({group_slug}) com permissão '{level}'...")

            members_url = f"https://api.bitbucket.org/2.0/workspaces/{WORKSPACE}/permissions/config/groups/{group_slug}/members"
            try:
                members = fetch_paginated_data(members_url)
            except ApiError as e:
                print(f"    [ERRO] Falha ao buscar membros do grupo '{group_name}' (HTTP {e.status_code}).")
                dados_repo.append([repo_slug, f"Grupo ({group_name})", f"ERRO AO ACESSAR MEMBROS (HTTP {e.status_code})", "-", level])
                continue

            if not members:
                print(f"    Grupo '{group_name}' não possui membros ou não foi possível acessar os membros.")
                dados_repo.append([repo_slug, "Grupo", group_name, "(Grupo Vazio)", level])
            else:
                print(f"    [OK] {len(members)} membro(s) encontrado(s) no grupo '{group_name}'.")
                for member in members:
                    print(f"    Adicionando membro '{member['display_name']}' do grupo '{group_name}' com permissão '{level}'...")
                    dados_repo.append([
                        repo_slug, f"Grupo ({group_name})", member['display_name'], f"@{member['nickname']}", level
                    ])

    # 3. Permissões herdadas do Projeto - buscadas sempre (mesmo se o repositório
    # em si não tiver nenhuma entrada própria), pois é comum o acesso vir só daqui.
    if project_key:
        linhas_projeto = coletar_dados_projeto(project_key, project_name)
        for tipo, nome, nickname, level in linhas_projeto:
            dados_repo.append([repo_slug, tipo, nome, nickname, level])

    # Se depois de checar repositório e projeto continuar tudo vazio, ainda assim
    # deixamos uma linha explícita - o repositório NUNCA deve simplesmente sumir
    # do relatório sem deixar rastro (esse era o bug original).
    if not dados_repo:
        print(f"  [AVISO] '{repo_slug}': nenhuma permissão encontrada no repositório nem no Projeto.")
        dados_repo.append([repo_slug, "N/A", "SEM PERMISSÕES CONFIGURADAS (repo e projeto vazios)", "-", "-"])

    return dados_repo

def main():
    # 1. Descoberta Automática de Repositórios via API
    repositorios = listar_todos_repositorios()
    print(f"Sucesso! Encontrados {len(repositorios)} repositórios mapeados no seu acesso.\n")

    if not repositorios:
        print("Nenhum repositório encontrado para este Workspace com suas credenciais atuais.")
        return

    # 2. Inicialização do arquivo Excel (Criando um novo do zero)
    wb = openpyxl.Workbook()
    ws_output = wb.active
    ws_output.title = "Permissões"

    headers = ["Repositório", "Tipo de Acesso", "Nome / Grupo", "Usuário (Nickname)", "Nível de Permissão"]
    ws_output.append(headers)

    # 3. Coleta de dados para cada repositório descoberto
    linhas_salvas = 0
    repos_com_erro = []
    repos_sem_acesso = []
    repos_sem_permissoes = []
    for i, repo in enumerate(repositorios, 1):
        slug = repo['slug']
        print(f"\n[{i}/{len(repositorios)}] Processando permissões de: {slug} (Projeto: {repo['project_name'] or 'N/A'})...")
        resultados = coletar_dados_repositorio(slug, repo['project_key'], repo['project_name'])

        if not resultados:
            # Não deve mais acontecer (todo repositório agora gera ao menos uma
            # linha), mas mantemos o aviso caso surja algum caso não previsto.
            print(f"  [AVISO] '{slug}' não retornou nenhuma linha - será omitido do relatório.")
            repos_sem_permissoes.append(slug)
        else:
            marcador = resultados[0][2]
            if isinstance(marcador, str) and marcador.startswith("ERRO AO ACESSAR"):
                repos_com_erro.append(slug)
            elif marcador in ("SEM ACESSO OU INACESSÍVEL", "SEM PERMISSÕES CONFIGURADAS (repo e projeto vazios)"):
                repos_sem_acesso.append(slug)

        for linha in resultados:
            ws_output.append(linha)
            linhas_salvas += 1

    # --- FORMATAÇÃO VISUAL DO EXCEL ---
    font_header = Font(name="Calibri", size=11, bold=True, color="FFFFFF")
    fill_header = PatternFill(start_color="1F4E78", end_color="1F4E78", fill_type="solid")
    align_center = Alignment(horizontal="center", vertical="center")
    align_left = Alignment(horizontal="left", vertical="center")
    thin_border = Border(
        left=Side(style='thin', color='D9D9D9'), right=Side(style='thin', color='D9D9D9'),
        top=Side(style='thin', color='D9D9D9'), bottom=Side(style='thin', color='D9D9D9')
    )
    fill_even = PatternFill(start_color="F9FAFB", end_color="F9FAFB", fill_type="solid")

    ws_output.row_dimensions[1].height = 25
    for col_idx, header in enumerate(headers, 1):
        cell = ws_output.cell(row=1, column=col_idx)
        cell.font = font_header
        cell.fill = fill_header
        cell.alignment = align_center
        cell.border = thin_border

    for r in range(2, ws_output.max_row + 1):
        ws_output.row_dimensions[r].height = 18
        for c in range(1, len(headers) + 1):
            cell = ws_output.cell(row=r, column=c)
            cell.border = thin_border
            if r % 2 == 0:
                cell.fill = fill_even
            if c in [2, 5]:
                cell.alignment = align_center
            else:
                cell.alignment = align_left

    larguras = {'A': 28, 'B': 20, 'C': 28, 'D': 25, 'E': 20}
    for col, width in larguras.items():
        ws_output.column_dimensions[col].width = width

    # Salva o resultado final no caminho definido
    wb.save(CAMINHO_EXCEL)
    print(f"\nPronto! O relatório foi gerado e salvo em '{CAMINHO_EXCEL}'.")
    print(f"Total de {linhas_salvas} linhas de permissões exportadas.")

    # --- RESUMO DE PROBLEMAS ---
    print("\n--- Resumo ---")
    print(f"Repositórios processados com sucesso: {len(repositorios) - len(repos_com_erro) - len(repos_sem_acesso) - len(repos_sem_permissoes)}")
    if repos_com_erro:
        print(f"Repositórios com ERRO de API (ver 'ERRO AO ACESSAR' na planilha, HTTP != 200/404): {len(repos_com_erro)}")
        for r in repos_com_erro:
            print(f"  - {r}")
    if repos_sem_acesso:
        print(f"Repositórios sem acesso (404 em permissions-config/users): {len(repos_sem_acesso)}")
        for r in repos_sem_acesso:
            print(f"  - {r}")
    if repos_sem_permissoes:
        print(f"Repositórios que não geraram nenhuma linha (inesperado): {len(repos_sem_permissoes)}")
        for r in repos_sem_permissoes:
            print(f"  - {r}")
    if not repos_com_erro and not repos_sem_acesso and not repos_sem_permissoes:
        print("Nenhum problema detectado - todos os repositórios foram processados normalmente.")

if __name__ == "__main__":
    main()