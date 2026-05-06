const KEY = "relay.token";
const USER_KEY = "relay.user";
const ROLE_KEY = "relay.role";

export function getToken(): string | null {
  return localStorage.getItem(KEY);
}
export function setToken(t: string) {
  localStorage.setItem(KEY, t);
}
export function clearAuth() {
  localStorage.removeItem(KEY);
  localStorage.removeItem(USER_KEY);
  localStorage.removeItem(ROLE_KEY);
}
export function setUser(name: string) {
  localStorage.setItem(USER_KEY, name);
}
export function getUser(): string | null {
  return localStorage.getItem(USER_KEY);
}
export function setRole(role: string) {
  localStorage.setItem(ROLE_KEY, role);
}
export function getRole(): string | null {
  return localStorage.getItem(ROLE_KEY);
}
