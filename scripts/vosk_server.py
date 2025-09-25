#!/usr/bin/env python3
"""
Vosk WebSocket Server for Anti-Fraud Voice Monitoring
ECPA-compliant automatic speech recognition service
"""

import asyncio
import json
import logging
import signal
import sys
import argparse
import time
import os
from pathlib import Path
from typing import Optional

import vosk
import websockets
from websockets.server import serve
from aiohttp import web, web_request
import soundfile as sf
import numpy as np
from prometheus_client import Counter, Histogram, Gauge, start_http_server

# Metrics for monitoring
TRANSCRIPTION_REQUESTS = Counter('vosk_transcription_requests_total', 'Total transcription requests')
TRANSCRIPTION_ERRORS = Counter('vosk_transcription_errors_total', 'Total transcription errors')
TRANSCRIPTION_DURATION = Histogram('vosk_transcription_duration_seconds', 'Time spent on transcription')
ACTIVE_CONNECTIONS = Gauge('vosk_active_connections', 'Number of active WebSocket connections')
MODEL_LOADED = Gauge('vosk_model_loaded', 'Whether Vosk model is loaded')

logger = logging.getLogger(__name__)

class VoskServer:
    def __init__(self, model_path: str, sample_rate: int = 8000):
        self.model_path = model_path
        self.sample_rate = sample_rate
        self.model: Optional[vosk.Model] = None
        self.running = False
        self.active_connections = set()

        # Set up logging
        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
        )

    async def load_model(self) -> bool:
        """Load the Vosk model"""
        try:
            logger.info(f"Loading Vosk model from {self.model_path}")
            if not Path(self.model_path).exists():
                logger.error(f"Model path does not exist: {self.model_path}")
                return False

            # Load model in executor to avoid blocking
            loop = asyncio.get_event_loop()
            self.model = await loop.run_in_executor(None, vosk.Model, self.model_path)

            logger.info("Vosk model loaded successfully")
            MODEL_LOADED.set(1)
            return True

        except Exception as e:
            logger.error(f"Failed to load Vosk model: {e}")
            MODEL_LOADED.set(0)
            return False

    async def handle_websocket(self, websocket, path):
        """Handle WebSocket connection for real-time transcription"""
        ACTIVE_CONNECTIONS.inc()
        self.active_connections.add(websocket)

        try:
            logger.info(f"New WebSocket connection from {websocket.remote_address}")

            if not self.model:
                await websocket.send(json.dumps({
                    "error": "Model not loaded",
                    "status": "error"
                }))
                return

            # Create recognizer for this connection
            recognizer = vosk.KaldiRecognizer(self.model, self.sample_rate)

            # Send connection confirmation
            await websocket.send(json.dumps({
                "status": "connected",
                "sample_rate": self.sample_rate,
                "model": "vosk-en-us"
            }))

            async for message in websocket:
                try:
                    TRANSCRIPTION_REQUESTS.inc()
                    start_time = time.time()

                    if isinstance(message, bytes):
                        # Audio data received
                        if recognizer.AcceptWaveform(message):
                            result = json.loads(recognizer.Result())
                            await websocket.send(json.dumps({
                                "type": "final",
                                "text": result.get("text", ""),
                                "confidence": result.get("conf", 0.0),
                                "timestamp": time.time()
                            }))
                        else:
                            partial = json.loads(recognizer.PartialResult())
                            await websocket.send(json.dumps({
                                "type": "partial",
                                "text": partial.get("partial", ""),
                                "timestamp": time.time()
                            }))
                    else:
                        # JSON control message
                        try:
                            data = json.loads(message)
                            if data.get("action") == "reset":
                                recognizer = vosk.KaldiRecognizer(self.model, self.sample_rate)
                                await websocket.send(json.dumps({
                                    "status": "reset",
                                    "timestamp": time.time()
                                }))
                        except json.JSONDecodeError:
                            logger.warning(f"Invalid JSON message: {message}")

                    # Record processing time
                    TRANSCRIPTION_DURATION.observe(time.time() - start_time)

                except Exception as e:
                    TRANSCRIPTION_ERRORS.inc()
                    logger.error(f"Error processing message: {e}")
                    await websocket.send(json.dumps({
                        "error": str(e),
                        "status": "error"
                    }))

        except websockets.exceptions.ConnectionClosedError:
            logger.info(f"WebSocket connection closed: {websocket.remote_address}")
        except Exception as e:
            logger.error(f"WebSocket error: {e}")
        finally:
            ACTIVE_CONNECTIONS.dec()
            if websocket in self.active_connections:
                self.active_connections.remove(websocket)

    async def health_check(self, request: web_request.Request) -> web.Response:
        """Health check endpoint"""
        status = {
            "status": "healthy" if self.model else "unhealthy",
            "model_loaded": bool(self.model),
            "active_connections": len(self.active_connections),
            "sample_rate": self.sample_rate,
            "timestamp": time.time()
        }

        if self.model:
            return web.json_response(status)
        else:
            return web.json_response(status, status=503)

    async def transcribe_file(self, request: web_request.Request) -> web.Response:
        """HTTP endpoint for file transcription"""
        try:
            if not self.model:
                return web.json_response({"error": "Model not loaded"}, status=503)

            # Get uploaded file
            reader = await request.multipart()
            field = await reader.next()

            if field.name != 'audio':
                return web.json_response({"error": "Expected 'audio' field"}, status=400)

            # Read audio data
            audio_data = await field.read()

            # Process with Vosk
            recognizer = vosk.KaldiRecognizer(self.model, self.sample_rate)

            start_time = time.time()
            TRANSCRIPTION_REQUESTS.inc()

            # Process audio in chunks
            chunk_size = 4000
            results = []

            for i in range(0, len(audio_data), chunk_size):
                chunk = audio_data[i:i + chunk_size]

                if recognizer.AcceptWaveform(chunk):
                    result = json.loads(recognizer.Result())
                    if result.get("text"):
                        results.append(result["text"])

            # Get final result
            final_result = json.loads(recognizer.FinalResult())
            if final_result.get("text"):
                results.append(final_result["text"])

            transcription = " ".join(results).strip()
            processing_time = time.time() - start_time

            TRANSCRIPTION_DURATION.observe(processing_time)

            return web.json_response({
                "transcription": transcription,
                "processing_time": processing_time,
                "status": "success"
            })

        except Exception as e:
            TRANSCRIPTION_ERRORS.inc()
            logger.error(f"File transcription error: {e}")
            return web.json_response({"error": str(e)}, status=500)

    async def start_server(self, host: str = "0.0.0.0", port: int = 2700, metrics_port: int = 9090):
        """Start the Vosk server"""
        logger.info("Starting Vosk server...")

        # Load model
        if not await self.load_model():
            logger.error("Failed to load model, exiting")
            sys.exit(1)

        # Start Prometheus metrics server
        start_http_server(metrics_port)
        logger.info(f"Metrics server started on port {metrics_port}")

        # Create HTTP app for health checks and file uploads
        app = web.Application()
        app.router.add_get('/health', self.health_check)
        app.router.add_post('/transcribe', self.transcribe_file)

        # Start HTTP server
        runner = web.AppRunner(app)
        await runner.setup()
        site = web.TCPSite(runner, host, port + 1)
        await site.start()

        # Start WebSocket server
        self.running = True
        logger.info(f"Vosk WebSocket server starting on {host}:{port}")
        logger.info(f"HTTP API available on {host}:{port + 1}")

        async with serve(self.handle_websocket, host, port):
            logger.info("Vosk server is running...")

            # Wait for shutdown signal
            while self.running:
                await asyncio.sleep(1)

        logger.info("Vosk server stopped")

    def stop(self):
        """Stop the server"""
        logger.info("Stopping Vosk server...")
        self.running = False

def signal_handler(server):
    """Handle shutdown signals"""
    def handler(signum, frame):
        logger.info(f"Received signal {signum}, shutting down...")
        server.stop()
    return handler

async def main():
    parser = argparse.ArgumentParser(description="Vosk WebSocket Server")
    parser.add_argument("--host", default="0.0.0.0", help="Host to bind to")
    parser.add_argument("--port", type=int, default=2700, help="Port to bind to")
    parser.add_argument("--model", required=True, help="Path to Vosk model")
    parser.add_argument("--sample-rate", type=int, default=8000, help="Sample rate")
    parser.add_argument("--metrics-port", type=int, default=9090, help="Prometheus metrics port")

    args = parser.parse_args()

    # Create and start server
    server = VoskServer(args.model, args.sample_rate)

    # Set up signal handlers
    signal.signal(signal.SIGINT, signal_handler(server))
    signal.signal(signal.SIGTERM, signal_handler(server))

    try:
        await server.start_server(args.host, args.port, args.metrics_port)
    except KeyboardInterrupt:
        logger.info("Received keyboard interrupt")
    except Exception as e:
        logger.error(f"Server error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(main())