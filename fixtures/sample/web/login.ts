export interface LoginPayload {
  username: string;
  password: string;
}

export async function submitLogin(payload: LoginPayload): Promise<boolean> {
  return payload.username.length > 0 && payload.password.length > 0;
}
