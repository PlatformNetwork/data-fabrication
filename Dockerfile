FROM python:3.12-slim

ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends docker-cli \
    && rm -rf /var/lib/apt/lists/*
COPY pyproject.toml ./
COPY src ./src
RUN pip install --no-cache-dir .
RUN useradd --create-home --shell /usr/sbin/nologin datafabrication \
    && mkdir -p /data \
    && chown -R datafabrication:datafabrication /app /data
USER datafabrication
ENV HOME=/home/datafabrication
EXPOSE 8080
CMD ["uvicorn", "data_fabrication.app:app", "--host", "0.0.0.0", "--port", "8080"]
