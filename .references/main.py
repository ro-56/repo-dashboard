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

def fetch_paginated_data(url):
    """Auxiliar para lidar com paginação da API do Bitbucket"""
    results = []
    while url:
        response = requests.get(url, auth=auth)
        if response.status_code == 404:
            return None
        if response.status_code != 200:
            print(f"Erro na API: {response.status_code} - {response.text}")
            break
        data = response.json()
        results.extend(data.get('values', []))
        url = data.get('next')
    return results

def listar_todos_repositorios():
    """Busca dinamicamente todos os repositórios do Workspace que o usuário tem acesso"""
    print(f"Buscando a lista de repositórios no Workspace '{WORKSPACE}'...")
    # Endpoint filtrado pelo Workspace para trazer todos os repositórios dele
    url = f"{BASE_URL}/{WORKSPACE}"
    repos_data = fetch_paginated_data(url)
    
    if not repos_data:
        return []
    
    # Extrai apenas o 'slug' (nome identificador na URL) de cada repositório
    slugs = [repo['slug'] for repo in repos_data]
    return slugs

def coletar_dados_repositorio(repo_slug):
    """Busca acessos diretos e por grupos de um repositório específico"""
    dados_repo = []
    url_base_repo = f"{BASE_URL}/{WORKSPACE}/{repo_slug}"

    # 1. Permissões Diretas de Usuários
    user_perms = fetch_paginated_data(f"{url_base_repo}/permissions-config/users")
    if user_perms is None:
        return [ [repo_slug, "N/A", "SEM ACESSO OU INACESSÍVEL", "-", "-"] ]

    for perm in user_perms:
        dados_repo.append([
            repo_slug, "Direto", perm['user']['display_name'], f"@{perm['user']['nickname']}", perm['permission'].upper()
        ])

    # 2. Permissões de Grupos e membros
    group_perms = fetch_paginated_data(f"{url_base_repo}/permissions-config/groups")
    for g_perm in group_perms:
        group_slug = g_perm['group']['slug']
        group_name = g_perm['group']['name']
        level = g_perm['permission'].upper()

        members_url = f"https://api.bitbucket.org/2.0/workspaces/{WORKSPACE}/permissions/config/groups/{group_slug}/members"
        members = fetch_paginated_data(members_url)

        if not members:
            dados_repo.append([repo_slug, "Grupo", group_name, "(Grupo Vazio)", level])
        else:
            for member in members:
                dados_repo.append([
                    repo_slug, f"Grupo ({group_name})", member['display_name'], f"@{member['nickname']}", level
                ])
                
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
    for repo in repositorios:
        print(f"Processando permissões de: {repo}...")
        resultados = coletar_dados_repositorio(repo)
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

if __name__ == "__main__":
    main()