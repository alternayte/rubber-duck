export interface RepoContext {
  id: string;
  session_id: string;
  name: string;
  source: string;
  local_path: string;
  created_at: string;
}

export interface FileNode {
  name: string;
  path: string;
  is_dir: boolean;
  children: FileNode[];
}

export interface FileSearchResult {
  repo_id: string;
  repo_name: string;
  relative_path: string;
  display: string;
}
