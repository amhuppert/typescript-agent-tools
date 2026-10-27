export async function fetchNewToken() {
    const /*#response*/response = await fetch("/api/token");
    const /*#status*/status = response.status;
    const /*#call*/pending = fetch(new Request("/api/token"));
    return { response, status, pending };
}
