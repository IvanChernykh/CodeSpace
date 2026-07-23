class SessionWorker:
    def validate(self, session: str) -> bool:
        return session.startswith("session:")


def process_login(session: str) -> bool:
    worker = SessionWorker()
    return worker.validate(session)
