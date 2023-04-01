import asyncio
from dataclasses import dataclass
import datetime
import logging
import sys
import time
import aiohttp
from yarl import URL

# https://twitter.com/BBCBweaking
#   -> https://nuwus.org -> https://nuwus.org/more_info.html
#   -> "Newsgathering, parsing, and tweeting takes place via zuzakistan/civilservant"
#   -> https://github.com/zuzakistan/civilservant/blob/master/plugins/news.js
#      ^ port of this file

logger = logging.getLogger(__name__)

@dataclass
class NewsAPI:
    url: URL

    def custom_decoder(self, data):
        return data

    async def request(self, session: aiohttp.ClientSession):
        resp = await session.get(self.url)
        try:
            data = await resp.json()
            logger.debug(f'{self.url=} {data=}')
        except:
            return
        try:
            val = self.custom_decoder(data)
        except:
            logger.exception('custom_decoder failed')
            return
        if val is None:
            return
        logger.info(f'got data from {self.url} | {val=!r} | ({data=!r})')


class BBCApi(NewsAPI):
    def custom_decoder(self, data):
        asset = data.get('asset')
        if len(asset) == 0:
            return None
        return asset

APIS: list[NewsAPI] = [
    BBCApi(URL('http://polling.bbc.co.uk/news/breaking-news/audience/domestic')),
    BBCApi(URL('http://polling.bbc.co.uk/news/breaking-news/audience/asia')),
    BBCApi(URL('http://polling.bbc.co.uk/news/breaking-news/audience/us')),
    BBCApi(URL('http://polling.bbc.co.uk/news/breaking-news/audience/international')),
    NewsAPI(URL('http://reuters.com/assets/breakingNews?view=json')),
]

POLL_TIMEOUT = 30

async def main():
    while True:
        logger.debug('polling...')
        start_time = time.time()
        async with aiohttp.ClientSession() as session:
            await asyncio.gather(*[api.request(session=session) for api in APIS])
        end_time = time.time()
        delta_time = start_time - end_time
        to_wait = max(0.0, POLL_TIMEOUT - delta_time)
        await asyncio.sleep(to_wait)


if __name__ == '__main__':
    # logging.basicConfig(
    #     level=logging.INFO,
    #     format='%(asctime)s %(levelname)-8s %(name)s %(message)s',
    #     datefmt='%Y-%m-%d %H:%M:%S',
    # )
    root_logger = logging.getLogger()
    root_logger.setLevel(logging.DEBUG)
    log_formatter = logging.Formatter('%(asctime)s %(levelname)-8s %(name)s %(message)s')
    file_log_handler = logging.FileHandler(filename='log.txt')
    file_log_handler.setFormatter(log_formatter)
    file_log_handler.setLevel(logging.DEBUG)
    root_logger.addHandler(file_log_handler)
    console_log_handler = logging.StreamHandler(stream=sys.stdout)
    console_log_handler.setFormatter(log_formatter)
    console_log_handler.setLevel(logging.INFO)
    root_logger.addHandler(console_log_handler)
    logger.info('starting!')
    asyncio.run(main())

