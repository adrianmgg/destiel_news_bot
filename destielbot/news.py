import asyncio
from dataclasses import dataclass
import datetime
import logging
import sys
import time
import aiohttp
from yarl import URL
from abc import ABC, abstractmethod

# https://github.com/zuzakistan/civilservant/blob/master/plugins/news.js
# https://github.com/zuzakistan/civilservant/blob/main/modules/news.js

logger = logging.getLogger(__name__)

@dataclass(frozen=True, slots=True, kw_only=True)
class NewsItem:
    headline: str
    id: str

@dataclass(frozen=True, slots=True)
class NewsAPI:
    url: URL

    @abstractmethod
    async def decode(self, response: aiohttp.ClientResponse) -> NewsItem | None:
        ...

    async def request(self, session: aiohttp.ClientSession):
        async with session.get(self.url) as resp:
            logger.debug(f'from {self.url!r} got {await resp.read()!r}')
            try:
                val = await self.decode(resp)
            except:
                logger.exception(f'decode failed! (raw response was {await resp.read()!r})')
                return
            if val is None:
                return
            logger.info(f'got data from {self.url!r} | {val=!r}')

class BBCApi(NewsAPI):
    async def decode(self, response: aiohttp.ClientResponse):
        match await response.json():
            case {'asset': {'headline': str(headline), 'assetUri': str(uri)}}:
                return NewsItem(headline=headline, id=f'BBC:{headline}:{uri}')
            case {'asset': {}}:
                return None
            case _:
                assert False  # TODO message

class ReutersApi(NewsAPI):
    async def decode(self, response: aiohttp.ClientResponse):
        # reuters sends an empty* response when there's no news so parsing will fail
        #  * not actually empty, it's '\n'
        try:
            data = await response.json()
        except:
            return
        if data is None:
            return
        match data:
            case {'headline': str(headline)}:
                # TODO there's some other logic in their version, check what's up with that
                return NewsItem(headline=headline, id=headline)
            case _:
                assert False  # TODO message

APIS: list[NewsAPI] = [
    BBCApi(URL('http://polling.bbc.co.uk/news/breaking-news/audience/domestic')),
    BBCApi(URL('http://polling.bbc.co.uk/news/breaking-news/audience/asia')),
    BBCApi(URL('http://polling.bbc.co.uk/news/breaking-news/audience/us')),
    BBCApi(URL('http://polling.bbc.co.uk/news/breaking-news/audience/international')),
    ReutersApi(URL('http://reuters.com/assets/breakingNews?view=json')),
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

