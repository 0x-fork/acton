import {
  ChevronLeft,
  ChevronRight,
  ChevronsRight,
  Download,
  ExternalLink,
  FileJson,
} from "lucide-react"
import {useNavigate, useParams} from "react-router-dom"
import {
  BlockChip,
  Button,
  CopyButton,
  CopyInlineAction,
  InlineActions,
  formatToncenterBlockId,
} from "@acton/ui"
import {useEffect, useMemo, useState} from "react"
import type {FC, ReactNode} from "react"

import type {TonClient} from "../api/client"
import type {V3Block, V3BlockId, V3TransactionListItem} from "../api/types"
import {ExplorerBreadcrumbs} from "../components/ExplorerBreadcrumbs"
import {
  DeveloperTransactionList,
  DeveloperTransactionListSkeleton,
} from "../components/DeveloperTransactionList"
import {ExplorerAddressChip} from "../components/ExplorerAddressChip"
import {hashToHex} from "../components/utils"
import {useAddressBook} from "../hooks/useAddressBook"
import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {useOpenExplorerPath, type ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useTransactionMessageNames} from "../hooks/useTransactionMessageNames"

import styles from "./BlocksPage.module.css"

const BLOCKS_PAGE_LIMIT = 8
const LAST_TRANSACTION_MESSAGES_LIMIT = 5
const LAST_TRANSACTIONS_FETCH_LIMIT = 12
const BLOCK_TRANSACTIONS_LIMIT = 100
const BLOCKS_REFRESH_MS = 2000
const MASTERCHAIN_SHARD = "8000000000000000"

interface BlocksPageProps {
  readonly client: TonClient
}

interface BlockDetailsPageProps extends BlocksPageProps {
  readonly latest?: boolean
}

interface BlocksPageState {
  readonly transactions: readonly V3TransactionListItem[]
  readonly masterchainBlocks: readonly V3Block[]
  readonly workchainBlocks: readonly V3Block[]
  readonly isLoading: boolean
  readonly error?: string
}

interface BlockDetailsState {
  readonly block?: V3Block
  readonly latestBlock?: V3Block
  readonly shardchainBlocks: readonly V3Block[]
  readonly transactions: readonly V3TransactionListItem[]
  readonly isLoading: boolean
  readonly error?: string
}

export const BlocksPage: FC<BlocksPageProps> = ({client}) => {
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {prefetchNames, updateDomains} = useAddressBook()
  const [state, setState] = useState<BlocksPageState>({
    transactions: [],
    masterchainBlocks: [],
    workchainBlocks: [],
    isLoading: true,
  })
  const {addresses, messageNamesByAddress} = useTransactionMessageNames(client, state.transactions)

  useEffect(() => {
    void prefetchNames(addresses)
  }, [addresses, prefetchNames])

  useEffect(() => {
    let isActive = true
    let timeoutId: ReturnType<typeof setTimeout> | undefined

    const loadBlocksPage = async (showLoading: boolean) => {
      if (showLoading) {
        setState(current => ({
          ...current,
          isLoading: true,
          error: undefined,
        }))
      }
      try {
        const [transactions, masterchainBlocks, workchainBlocks] = await Promise.all([
          client.getRecentTransactions(LAST_TRANSACTIONS_FETCH_LIMIT),
          client.getBlocks({
            workchain: -1,
            limit: BLOCKS_PAGE_LIMIT,
            sort: "desc",
          }),
          client.getBlocks({
            workchain: 0,
            limit: BLOCKS_PAGE_LIMIT,
            sort: "desc",
          }),
        ])

        if (!isActive) {
          return
        }

        updateDomains(transactions.address_book)
        setState({
          transactions: transactions.transactions,
          masterchainBlocks: masterchainBlocks.blocks,
          workchainBlocks: workchainBlocks.blocks,
          isLoading: false,
        })
      } catch (error) {
        if (!isActive) {
          return
        }
        setState(current => ({
          ...current,
          isLoading: false,
          error:
            current.masterchainBlocks.length === 0 && current.workchainBlocks.length === 0
              ? error instanceof Error
                ? error.message
                : "Failed to load blocks"
              : undefined,
        }))
      } finally {
        if (isActive) {
          timeoutId = globalThis.setTimeout(() => void loadBlocksPage(false), BLOCKS_REFRESH_MS)
        }
      }
    }

    void loadBlocksPage(true)

    return () => {
      isActive = false
      if (timeoutId !== undefined) {
        globalThis.clearTimeout(timeoutId)
      }
    }
  }, [client, updateDomains])

  return (
    <div className={styles.container}>
      <ExplorerBreadcrumbs items={[{label: "Blocks"}]} />
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>Blocks</h1>
        </div>
      </section>

      <section className={styles.blocksLayout}>
        {state.error ? (
          <TableStateBlock>{state.error}</TableStateBlock>
        ) : state.isLoading ? (
          <DeveloperTransactionListSkeleton
            className={styles.blocksTransactionsTable}
            title="Last transactions"
            rows={LAST_TRANSACTION_MESSAGES_LIMIT}
          />
        ) : (
          <DeveloperTransactionList
            className={styles.blocksTransactionsTable}
            title="Last transactions"
            transactions={state.transactions}
            maxRows={LAST_TRANSACTION_MESSAGES_LIMIT}
            messageNamesByAddress={messageNamesByAddress}
            onTransactionClick={(hashHex, _transaction, event) => {
              openPath(routes.transactionPath(hashHex), event)
            }}
            onAddressClick={(address, event) => {
              openPath(routes.addressPath(address), event)
            }}
          />
        )}

        <div className={styles.blocksTableGrid}>
          <BlockTableSection
            title="Last masterchain blocks"
            blocks={state.masterchainBlocks}
            isLoading={state.isLoading}
            emptyLabel="No masterchain blocks yet"
            onOpenBlock={(block, event) => openPath(blockPath(block), event)}
          />
          <BlockTableSection
            title="Last workchain blocks"
            blocks={state.workchainBlocks}
            isLoading={state.isLoading}
            emptyLabel="No workchain blocks yet"
            onOpenBlock={(block, event) => openPath(blockPath(block), event)}
          />
        </div>
      </section>
    </div>
  )
}

export const BlockDetailsPage: FC<BlockDetailsPageProps> = ({client, latest = false}) => {
  const params = useParams<{
    workchain: string
    shard: string
    seqno: string
  }>()
  const navigate = useNavigate()
  const {network} = useNetworkInfo()
  const routes = useExplorerRoutePaths()
  const openPath = useOpenExplorerPath()
  const {prefetchNames, updateDomains} = useAddressBook()
  const routeWorkchain = Number(params.workchain)
  const routeShard = params.shard ?? ""
  const routeSeqno = Number(params.seqno)
  const [state, setState] = useState<BlockDetailsState>({
    shardchainBlocks: [],
    transactions: [],
    isLoading: true,
  })

  useEffect(() => {
    let isActive = true

    const loadBlockDetails = async () => {
      if (
        !latest &&
        (!Number.isInteger(routeWorkchain) || !Number.isInteger(routeSeqno) || !routeShard)
      ) {
        setState({
          shardchainBlocks: [],
          transactions: [],
          isLoading: false,
          error: "Invalid block route.",
        })
        return
      }

      setState(current => ({
        ...current,
        isLoading: true,
        error: undefined,
      }))
      try {
        const [blockResponse, latestResponse] = await Promise.all([
          latest
            ? Promise.resolve({blocks: []})
            : client.getBlocks({
                workchain: routeWorkchain,
                shard: routeShard,
                seqno: routeSeqno,
                limit: 1,
              }),
          latest
            ? client.getBlocks({workchain: -1, limit: 1, sort: "desc"})
            : client.getBlocks({
                workchain: routeWorkchain,
                shard: routeShard,
                limit: 1,
                sort: "desc",
              }),
        ])
        const block = latest
          ? latestResponse.blocks[0]
          : blockResponse.blocks.find(candidate =>
              isSameBlock(candidate, routeWorkchain, routeShard, routeSeqno),
            )

        if (!block) {
          if (isActive) {
            setState({
              latestBlock: latestResponse.blocks[0],
              shardchainBlocks: [],
              transactions: [],
              isLoading: false,
              error: "Block not found.",
            })
          }
          return
        }

        const [transactionsResponse, shardchainResponse] = await Promise.all([
          client.getBlockTransactions({
            workchain: block.workchain,
            shard: block.shard,
            seqno: block.seqno,
            limit: BLOCK_TRANSACTIONS_LIMIT,
          }),
          block.workchain === -1
            ? client.getMasterchainBlockShards(block.seqno)
            : Promise.resolve({blocks: []}),
        ])

        if (!isActive) {
          return
        }

        updateDomains(transactionsResponse.address_book)
        setState({
          block,
          latestBlock: latestResponse.blocks[0],
          shardchainBlocks: shardchainResponse.blocks,
          transactions: transactionsResponse.transactions,
          isLoading: false,
        })
      } catch (error) {
        if (!isActive) {
          return
        }
        setState(current => ({
          ...current,
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load block",
        }))
      }
    }

    void loadBlockDetails()

    return () => {
      isActive = false
    }
  }, [client, latest, routeShard, routeSeqno, routeWorkchain, updateDomains])

  const workchain = latest ? (state.block?.workchain ?? -1) : routeWorkchain
  const shard = latest ? (state.block?.shard ?? MASTERCHAIN_SHARD) : routeShard
  const seqno = latest ? (state.block?.seqno ?? Number.NaN) : routeSeqno

  const title = workchain === -1 ? "Masterchain block" : "Workchain block"
  const hasResolvedBlockId =
    Number.isInteger(workchain) && Number.isInteger(seqno) && Boolean(shard)
  const hasValidRoute = latest || hasResolvedBlockId
  const blockId = hasResolvedBlockId ? formatToncenterBlockId({workchain, shard, seqno}) : undefined
  const latestPath = state.latestBlock ? blockPath(state.latestBlock) : undefined
  const canOpenPrev = hasResolvedBlockId && seqno > 1
  const prevPath = canOpenPrev ? blockPath({workchain, shard, seqno: seqno - 1}) : undefined
  const nextPath = hasResolvedBlockId ? blockPath({workchain, shard, seqno: seqno + 1}) : undefined
  const transactionAddresses = useMemo(
    () => state.transactions.map(transaction => transaction.account),
    [state.transactions],
  )
  const blockActions = state.block ? getBlockActions(state.block, network.testOnly) : undefined

  useEffect(() => {
    void prefetchNames(transactionAddresses)
  }, [prefetchNames, transactionAddresses])

  return (
    <div className={styles.container}>
      <ExplorerBreadcrumbs
        items={[
          {label: "Blocks", path: routes.blocksPath},
          {
            label: blockId ?? title,
            copy: blockId
              ? {
                  value: blockId,
                  label: "Copy block ID",
                  copiedLabel: "Block ID copied",
                }
              : undefined,
          },
        ]}
      />
      <section className={styles.hero}>
        <div>
          <h1 className={styles.title}>{title}</h1>
        </div>
      </section>

      <section className={styles.blocksLayout}>
        {hasValidRoute ? (
          <div className={styles.blockDetailControls}>
            <div className={styles.blockDetailToolbar} aria-label="Block navigation">
              <Button
                type="button"
                variant="outline"
                size="sm"
                leadingIcon={<ChevronLeft size={14} />}
                disabled={!prevPath}
                onClick={() => prevPath && void navigate(prevPath)}
              >
                Prev block
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                trailingIcon={<ChevronRight size={14} />}
                disabled={!nextPath}
                onClick={() => nextPath && void navigate(nextPath)}
              >
                Next block
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                trailingIcon={<ChevronsRight size={14} />}
                disabled={
                  !latestPath ||
                  (state.block !== undefined && latestPath === blockPath(state.block))
                }
                onClick={() => latestPath && void navigate(latestPath)}
              >
                Latest
              </Button>
            </div>

            {state.block && blockActions ? (
              <div className={styles.blockHeaderActions} aria-label="Block actions">
                <a
                  className={styles.blockActionLink}
                  href={blockActions.downloadUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  <Download size={15} aria-hidden="true" />
                  Download
                </a>
                {blockActions.configUrl ? (
                  <a
                    className={styles.blockActionLink}
                    href={blockActions.configUrl}
                    target="_blank"
                    rel="noreferrer"
                  >
                    <FileJson size={15} aria-hidden="true" />
                    Config
                  </a>
                ) : (
                  <span className={`${styles.blockActionLink} ${styles.blockActionLinkDisabled}`}>
                    <FileJson size={15} aria-hidden="true" />
                    Config
                  </span>
                )}
                <CopyButton
                  value={blockActions.extendedBlockId}
                  label="Copy extended block ID"
                  copiedLabel="Extended block ID copied"
                  copiedChildren="Copied ID"
                  variant="outline"
                  size="sm"
                >
                  Copy block ID
                </CopyButton>
                <span className={styles.blockActionSeparator} aria-hidden="true" />
                <a
                  className={styles.blockExplorerLink}
                  href={blockActions.tonscanUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  Tonscan
                  <ExternalLink size={13} aria-hidden="true" />
                </a>
                <a
                  className={styles.blockExplorerLink}
                  href={blockActions.toncoinUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  toncoin.org
                  <ExternalLink size={13} aria-hidden="true" />
                </a>
              </div>
            ) : null}
          </div>
        ) : null}

        {state.error ? (
          <TableStateBlock>{state.error}</TableStateBlock>
        ) : state.isLoading || !state.block ? (
          <BlockDetailsSkeleton showShardchainBlocks={workchain === -1} />
        ) : (
          <>
            <BlockSummaryTable
              block={state.block}
              onOpenBlock={(block, event) => openPath(blockPath(block), event)}
            />

            {state.block.workchain === -1 ? (
              <BlockTableSection
                title="Last shard blocks"
                blocks={state.shardchainBlocks}
                isLoading={false}
                emptyLabel="No shardchain blocks for this masterchain block"
                showShardFlags
                onOpenBlock={(block, event) => openPath(blockPath(block), event)}
              />
            ) : null}

            <BlockTransactionsTable
              transactions={state.transactions}
              onOpenAccount={(address, event) => openPath(routes.addressPath(address), event)}
              onOpenTransaction={(hash, event) =>
                openPath(routes.transactionPath(hashToHex(hash) ?? hash), event)
              }
            />
          </>
        )}
      </section>
    </div>
  )
}

const BlockTableSection: FC<{
  readonly title: string
  readonly blocks: readonly V3Block[]
  readonly isLoading: boolean
  readonly emptyLabel: string
  readonly showShardFlags?: boolean
  readonly onOpenBlock: (block: V3Block, event?: ExplorerNavigationClickEvent) => void
}> = ({title, blocks, isLoading, emptyLabel, showShardFlags = false, onOpenBlock}) => {
  if (isLoading) {
    return <BlockTableSkeleton title={title} rows={4} showShardFlags={showShardFlags} />
  }

  if (blocks.length === 0) {
    return <TableStateBlock title={title}>{emptyLabel}</TableStateBlock>
  }

  return (
    <section className={styles.blocksTableFrame} aria-label={title}>
      <header className={styles.blocksTableTitle}>{title}</header>
      <div className={styles.blocksTableScroller}>
        <table className={`${styles.blocksTable} ${showShardFlags ? styles.shardBlocksTable : ""}`}>
          <thead>
            <tr>
              <th>Block</th>
              <th>Transactions</th>
              <th>Generated at</th>
              {showShardFlags ? (
                <>
                  <th>Before split</th>
                  <th>After split</th>
                  <th>Want split</th>
                  <th>Want merge</th>
                </>
              ) : null}
            </tr>
          </thead>
          <tbody>
            {blocks.map(block => (
              <tr
                key={formatToncenterBlockId(block)}
                className={styles.blocksTableRow}
                tabIndex={0}
                onClick={event => onOpenBlock(block, event)}
                onKeyDown={event => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault()
                    onOpenBlock(block)
                  }
                }}
              >
                <td className={styles.blocksPrimaryCell}>
                  <BlockChip
                    workchain={block.workchain}
                    shard={block.shard}
                    seqno={block.seqno}
                    href={blockPath(block)}
                    onClick={event => {
                      event.stopPropagation()
                      onOpenBlock(block, event)
                    }}
                  />
                </td>
                <td>{block.tx_count.toLocaleString()}</td>
                <td
                  title={formatAbsoluteBlockTime(block)}
                  data-visual-dynamic="time"
                  data-visual-placeholder="<time>"
                >
                  {formatAbsoluteBlockTime(block)}
                </td>
                {showShardFlags ? (
                  <>
                    <td>
                      <BooleanValue value={block.before_split} />
                    </td>
                    <td>
                      <BooleanValue value={block.after_split} />
                    </td>
                    <td>
                      <BooleanValue value={block.want_split} />
                    </td>
                    <td>
                      <BooleanValue value={block.want_merge} />
                    </td>
                  </>
                ) : null}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

const BlockTransactionsTable: FC<{
  readonly transactions: readonly V3TransactionListItem[]
  readonly onOpenAccount: (address: string, event?: ExplorerNavigationClickEvent) => void
  readonly onOpenTransaction: (hash: string, event?: ExplorerNavigationClickEvent) => void
}> = ({transactions, onOpenAccount, onOpenTransaction}) => {
  if (transactions.length === 0) {
    return <TableStateBlock title="Transactions">No transactions in this block</TableStateBlock>
  }

  return (
    <section className={styles.blocksTableFrame} aria-label="Transactions">
      <header className={styles.blocksTableTitle}>Transactions</header>
      <div className={styles.blocksTableScroller}>
        <table className={`${styles.blocksTable} ${styles.blockTransactionsTable}`}>
          <thead>
            <tr>
              <th>#</th>
              <th>Account</th>
              <th>Logical time</th>
              <th>Hash</th>
              <th>Exit code</th>
            </tr>
          </thead>
          <tbody>
            {transactions.map((transaction, index) => {
              const hash = hashToHex(transaction.hash) ?? transaction.hash
              return (
                <tr
                  key={`${transaction.hash}:${transaction.lt}`}
                  className={styles.blocksTableRow}
                  tabIndex={0}
                  onClick={event => onOpenTransaction(transaction.hash, event)}
                  onKeyDown={event => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault()
                      onOpenTransaction(transaction.hash)
                    }
                  }}
                >
                  <td>{index + 1}</td>
                  <td>
                    <ExplorerAddressChip
                      address={transaction.account}
                      fallback="Account"
                      onAddressClick={onOpenAccount}
                    />
                  </td>
                  <td>{transaction.lt}</td>
                  <td>
                    <span className={styles.blocksHashCell}>
                      <span className={styles.blocksHashText} title={hash}>
                        {compactMiddle(hash, 18)}
                      </span>
                      <CopyInlineAction
                        value={hash}
                        size="compact"
                        label="Copy transaction hash"
                        copiedLabel="Transaction hash copied"
                      />
                    </span>
                  </td>
                  <td className={styles.blocksExitCodeCell}>
                    {formatTransactionExitCode(transaction)}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </section>
  )
}

const BlockTableSkeleton: FC<{
  readonly title: string
  readonly rows: number
  readonly showShardFlags?: boolean
}> = ({title, rows, showShardFlags = false}) => (
  <section className={styles.blocksTableFrame} aria-label={`Loading ${title}`}>
    <header className={styles.blocksTableTitle}>{title}</header>
    <div className={styles.blocksTableScroller}>
      <table className={`${styles.blocksTable} ${showShardFlags ? styles.shardBlocksTable : ""}`}>
        <thead>
          <tr>
            <th>Block</th>
            <th>Transactions</th>
            <th>Generated at</th>
            {showShardFlags ? (
              <>
                <th>Before split</th>
                <th>After split</th>
                <th>Want split</th>
                <th>Want merge</th>
              </>
            ) : null}
          </tr>
        </thead>
        <tbody>
          {Array.from({length: rows}, (_, index) => (
            <tr key={`block-table-skeleton-${index}`}>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonBlock}`} />
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonCount}`} />
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonTime}`} />
              </td>
              {showShardFlags
                ? Array.from({length: 4}, (_, flagIndex) => (
                    <td key={`block-table-skeleton-${index}-flag-${flagIndex}`}>
                      <span className={`${styles.skeletonLine} ${styles.blocksSkeletonFlag}`} />
                    </td>
                  ))
                : null}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  </section>
)

const BlockTransactionsTableSkeleton: FC<{readonly rows: number}> = ({rows}) => (
  <section className={styles.blocksTableFrame} aria-label="Loading transactions">
    <header className={styles.blocksTableTitle}>Transactions</header>
    <div className={styles.blocksTableScroller}>
      <table className={`${styles.blocksTable} ${styles.blockTransactionsTable}`}>
        <thead>
          <tr>
            <th>#</th>
            <th>Account</th>
            <th>Logical time</th>
            <th>Hash</th>
            <th>Exit code</th>
          </tr>
        </thead>
        <tbody>
          {Array.from({length: rows}, (_, index) => (
            <tr key={`block-transaction-skeleton-${index}`}>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonIndex}`} />
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonAccount}`} />
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonLt}`} />
              </td>
              <td>
                <span className={styles.blocksSkeletonHashCell}>
                  <span className={`${styles.skeletonLine} ${styles.blocksSkeletonHash}`} />
                  <span className={`${styles.skeletonLine} ${styles.blocksSkeletonCopy}`} />
                </span>
              </td>
              <td>
                <span className={`${styles.skeletonLine} ${styles.blocksSkeletonExitCode}`} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  </section>
)

const BlockSummaryTable: FC<{
  readonly block: V3Block
  readonly onOpenBlock: (block: V3BlockId, event: ExplorerNavigationClickEvent) => void
}> = ({block, onOpenBlock}) => {
  const rootHash = formatBlockHash(block.root_hash)
  const fileHash = formatBlockHash(block.file_hash)
  const createdBy = formatBlockHash(block.created_by)
  const randSeed = formatBlockHash(block.rand_seed)
  const masterchainShard = block.masterchain_block_ref?.shard ?? MASTERCHAIN_SHARD
  const prevKeyBlockSeqno = block.prev_key_block_seqno
  const minRefMcSeqno = block.min_ref_mc_seqno
  const hasGenSoftware =
    block.gen_software_version !== undefined || block.gen_software_capabilities !== undefined

  return (
    <section className={styles.blockDetailsPanel} aria-label="Block details">
      <BlockDetailSection label="Identity" contentClassName={styles.blockFourColumnGrid}>
        <BlockDetailItem label="Workchain" value={block.workchain.toString()} />
        <BlockDetailItem label="Shard" value={block.shard} mono />
        <BlockDetailItem label="Seqno" value={block.seqno.toString()} mono />
        <BlockDetailItem label="Global ID" value={formatOptionalNumber(block.global_id)} mono />
      </BlockDetailSection>

      <BlockDetailSection label="Hashes" contentClassName={styles.blockHashesGrid}>
        <BlockDetailItem label="Root hash" value={rootHash} copyValue={rootHash} mono />
        <BlockDetailItem label="File hash" value={fileHash} copyValue={fileHash} mono />
        <BlockDetailItem label="Created by" value={createdBy} copyValue={createdBy} mono />
        <BlockDetailItem label="Rand seed" value={randSeed} copyValue={randSeed} mono />
      </BlockDetailSection>

      <BlockDetailSection
        label="Generation"
        contentClassName={hasGenSoftware ? undefined : styles.blockFourColumnGrid}
      >
        <BlockDetailItem
          label="Gen utime"
          value={formatAbsoluteBlockTime(block)}
          title={formatAbsoluteBlockTime(block)}
          visualPlaceholder="<time>"
        />
        <BlockDetailItem label="Version" value={formatOptionalNumber(block.version)} mono />
        <BlockDetailItem label="Vert seqno" value={formatOptionalNumber(block.vert_seqno)} mono />
        <BlockDetailItem
          label="Gen catchain seqno"
          value={formatOptionalNumber(block.gen_catchain_seqno)}
          mono
        />
        {block.gen_software_version === undefined ? null : (
          <BlockDetailItem
            label="Gen software version"
            value={formatOptionalNumber(block.gen_software_version)}
            mono
          />
        )}
        {block.gen_software_capabilities === undefined ? null : (
          <BlockDetailItem
            label="Gen software capabilities"
            value={formatOptionalNumber(block.gen_software_capabilities)}
            mono
          />
        )}
      </BlockDetailSection>

      <BlockDetailSection label="References">
        <BlockDetailItem
          label="Prev refs"
          value={
            block.prev_blocks && block.prev_blocks.length > 0 ? (
              <span className={styles.blockReferenceList}>
                {block.prev_blocks.map(ref => (
                  <BlockChip
                    key={formatToncenterBlockId(ref)}
                    workchain={ref.workchain}
                    shard={ref.shard}
                    seqno={ref.seqno}
                    display="full"
                    href={blockPath(ref)}
                    onClick={event => onOpenBlock(ref, event)}
                  />
                ))}
              </span>
            ) : (
              "None"
            )
          }
        />
        <BlockDetailItem
          label="Prev key block seqno"
          value={
            prevKeyBlockSeqno && prevKeyBlockSeqno > 0 ? (
              <BlockChip
                workchain={-1}
                shard={masterchainShard}
                seqno={prevKeyBlockSeqno}
                href={blockPath({
                  workchain: -1,
                  shard: masterchainShard,
                  seqno: prevKeyBlockSeqno,
                })}
                onClick={event =>
                  onOpenBlock(
                    {
                      workchain: -1,
                      shard: masterchainShard,
                      seqno: prevKeyBlockSeqno,
                    },
                    event,
                  )
                }
              />
            ) : (
              formatOptionalNumber(prevKeyBlockSeqno)
            )
          }
        />
        <BlockDetailItem
          label="Min ref mc seqno"
          value={
            minRefMcSeqno && minRefMcSeqno > 0 ? (
              <BlockChip
                workchain={-1}
                shard={masterchainShard}
                seqno={minRefMcSeqno}
                href={blockPath({
                  workchain: -1,
                  shard: masterchainShard,
                  seqno: minRefMcSeqno,
                })}
                onClick={event =>
                  onOpenBlock(
                    {
                      workchain: -1,
                      shard: masterchainShard,
                      seqno: minRefMcSeqno,
                    },
                    event,
                  )
                }
              />
            ) : (
              formatOptionalNumber(minRefMcSeqno)
            )
          }
        />
      </BlockDetailSection>

      <BlockDetailSection label="Activity">
        <BlockDetailItem label="Tx quantity" value={block.tx_count.toLocaleString()} mono />
        {block.in_msg_descr_length === undefined ? null : (
          <BlockDetailItem
            label="In msg descr length"
            value={block.in_msg_descr_length.toLocaleString()}
            mono
          />
        )}
        {block.out_msg_descr_length === undefined ? null : (
          <BlockDetailItem
            label="Out msg descr length"
            value={block.out_msg_descr_length.toLocaleString()}
            mono
          />
        )}
      </BlockDetailSection>

      <BlockDetailSection label="Flags" contentClassName={styles.blockSixColumnGrid}>
        <BlockDetailItem label="Key block" value={<BooleanValue value={block.key_block} />} />
        <BlockDetailItem label="After merge" value={<BooleanValue value={block.after_merge} />} />
        <BlockDetailItem label="After split" value={<BooleanValue value={block.after_split} />} />
        <BlockDetailItem label="Before split" value={<BooleanValue value={block.before_split} />} />
        <BlockDetailItem label="Want merge" value={<BooleanValue value={block.want_merge} />} />
        <BlockDetailItem label="Want split" value={<BooleanValue value={block.want_split} />} />
      </BlockDetailSection>

      <BlockDetailSection label="Logical time">
        <BlockDetailItem label="Start LT / End LT" value={`${block.start_lt} – ${block.end_lt}`} />
      </BlockDetailSection>
    </section>
  )
}

const BlockDetailSection: FC<{
  readonly label: string
  readonly children: ReactNode
  readonly contentClassName?: string
}> = ({label, children, contentClassName}) => (
  <div className={styles.blockDetailRow}>
    <div className={styles.blockDetailLabel}>{label}</div>
    <div className={`${styles.blockDetailGrid} ${contentClassName ?? ""}`}>{children}</div>
  </div>
)

interface BlockDetailItemProps {
  readonly label: string
  readonly value: ReactNode
  readonly title?: string
  readonly copyValue?: string
  readonly mono?: boolean
  readonly visualPlaceholder?: string
}

const BlockDetailItem: FC<BlockDetailItemProps> = ({
  label,
  value,
  title,
  copyValue,
  mono = false,
  visualPlaceholder,
}) => (
  <div className={styles.blockDetailItem}>
    <span className={styles.blockDetailItemLabel}>{label}</span>
    <span
      className={`${styles.blockDetailValue} ${mono ? styles.blocksMonoCell : ""}`}
      title={title ?? (typeof value === "string" ? value : undefined)}
      data-visual-dynamic={visualPlaceholder ? "time" : undefined}
      data-visual-placeholder={visualPlaceholder}
    >
      {copyValue ? (
        <InlineActions
          className={styles.blockDetailInlineActions}
          visibility="hover"
          actions={
            <CopyInlineAction
              value={copyValue}
              size="compact"
              label={`Copy ${label.toLowerCase()}`}
              copiedLabel={`${label} copied`}
            />
          }
        >
          <span className={styles.blockDetailValueText}>{value}</span>
        </InlineActions>
      ) : (
        value
      )}
    </span>
  </div>
)

const BooleanValue: FC<{readonly value: boolean | undefined}> = ({value}) => {
  if (value === undefined) {
    return <>—</>
  }
  return (
    <span className={value ? styles.blockBooleanTrue : styles.blockBooleanFalse}>
      {value ? "true" : "false"}
    </span>
  )
}

const BlockDetailSkeletonItem: FC<{
  readonly label: string
  readonly valueClassName?: string
  readonly wide?: boolean
}> = ({label, valueClassName, wide = false}) => (
  <div className={styles.blockDetailItem}>
    <span className={styles.blockDetailItemLabel}>{label}</span>
    <span
      className={`${styles.skeletonLine} ${wide ? styles.blockDetailSkeletonValueWide : styles.blockDetailSkeletonValue} ${valueClassName ?? ""}`}
    />
  </div>
)

const BlockDetailsSkeleton: FC<{readonly showShardchainBlocks: boolean}> = ({
  showShardchainBlocks,
}) => (
  <>
    <section className={styles.blockDetailsPanel} aria-label="Loading block details">
      <BlockDetailSection label="Identity" contentClassName={styles.blockFourColumnGrid}>
        <BlockDetailSkeletonItem label="Workchain" />
        <BlockDetailSkeletonItem label="Shard" />
        <BlockDetailSkeletonItem label="Seqno" />
        <BlockDetailSkeletonItem label="Global ID" />
      </BlockDetailSection>

      <BlockDetailSection label="Hashes" contentClassName={styles.blockHashesGrid}>
        <BlockDetailSkeletonItem
          label="Root hash"
          valueClassName={styles.blockDetailSkeletonHashValue}
          wide
        />
        <BlockDetailSkeletonItem
          label="File hash"
          valueClassName={styles.blockDetailSkeletonHashValue}
          wide
        />
        <BlockDetailSkeletonItem
          label="Created by"
          valueClassName={styles.blockDetailSkeletonHashValue}
          wide
        />
        <BlockDetailSkeletonItem
          label="Rand seed"
          valueClassName={styles.blockDetailSkeletonHashValue}
          wide
        />
      </BlockDetailSection>

      <BlockDetailSection label="Generation" contentClassName={styles.blockFourColumnGrid}>
        <BlockDetailSkeletonItem label="Gen utime" />
        <BlockDetailSkeletonItem label="Version" />
        <BlockDetailSkeletonItem label="Vert seqno" />
        <BlockDetailSkeletonItem label="Gen catchain seqno" />
      </BlockDetailSection>

      <BlockDetailSection label="References">
        <BlockDetailSkeletonItem
          label="Prev refs"
          valueClassName={styles.blockDetailSkeletonChipValue}
          wide
        />
        <BlockDetailSkeletonItem
          label="Prev key block seqno"
          valueClassName={styles.blockDetailSkeletonChipValue}
        />
        <BlockDetailSkeletonItem
          label="Min ref mc seqno"
          valueClassName={styles.blockDetailSkeletonChipValue}
        />
      </BlockDetailSection>

      <BlockDetailSection label="Activity">
        <BlockDetailSkeletonItem label="Tx quantity" />
      </BlockDetailSection>

      <BlockDetailSection label="Flags" contentClassName={styles.blockSixColumnGrid}>
        <BlockDetailSkeletonItem label="Key block" />
        <BlockDetailSkeletonItem label="After merge" />
        <BlockDetailSkeletonItem label="After split" />
        <BlockDetailSkeletonItem label="Before split" />
        <BlockDetailSkeletonItem label="Want merge" />
        <BlockDetailSkeletonItem label="Want split" />
      </BlockDetailSection>

      <BlockDetailSection label="Logical time">
        <BlockDetailSkeletonItem label="Start LT / End LT" wide />
      </BlockDetailSection>
    </section>
    {showShardchainBlocks ? (
      <BlockTableSkeleton title="Last shard blocks" rows={1} showShardFlags />
    ) : null}
    <BlockTransactionsTableSkeleton rows={4} />
  </>
)

const TableStateBlock: FC<{
  readonly title?: string
  readonly children: ReactNode
}> = ({title, children}) => (
  <section className={styles.blocksTableFrame}>
    {title ? <header className={styles.blocksTableTitle}>{title}</header> : null}
    <div className={styles.blocksTableState}>{children}</div>
  </section>
)

function blockPath(block: Pick<V3Block, "workchain" | "shard" | "seqno">): string {
  return `/block/${block.workchain}/${encodeURIComponent(block.shard)}/${block.seqno}`
}

function isSameBlock(block: V3Block, workchain: number, shard: string, seqno: number): boolean {
  return block.workchain === workchain && block.shard === shard && block.seqno === seqno
}

function formatBlockHash(value: string): string {
  return hashToHex(value) ?? value
}

function formatOptionalNumber(value: number | undefined): string {
  return value === undefined ? "—" : value.toString()
}

function getBlockActions(
  block: V3Block,
  testOnly: boolean,
): {
  readonly downloadUrl: string
  readonly configUrl?: string
  readonly tonscanUrl: string
  readonly toncoinUrl: string
  readonly extendedBlockId: string
} {
  const blockId = formatToncenterBlockId(block)
  const rootHash = formatBlockHash(block.root_hash)
  const fileHash = formatBlockHash(block.file_hash)
  const tonapiOrigin = testOnly ? "https://testnet.tonapi.io" : "https://tonapi.io"
  const tonviewerOrigin = testOnly ? "https://testnet.tonviewer.com" : "https://tonviewer.com"
  const tonscanOrigin = testOnly ? "https://testnet.tonscan.org" : "https://tonscan.org"
  const toncoinOrigin = testOnly
    ? "https://test-explorer.toncoin.org"
    : "https://explorer.toncoin.org"
  return {
    downloadUrl: `${tonapiOrigin}/v2/blockchain/blocks/${encodeURIComponent(blockId)}/boc`,
    configUrl:
      block.prev_key_block_seqno && block.prev_key_block_seqno > 0
        ? `${tonviewerOrigin}/config/${block.prev_key_block_seqno}`
        : undefined,
    tonscanUrl: `${tonscanOrigin}/block/${block.workchain}:${block.shard}:${block.seqno}`,
    toncoinUrl: `${toncoinOrigin}/search?workchain=${block.workchain}&shard=${encodeURIComponent(block.shard)}&seqno=${block.seqno}`,
    extendedBlockId: `(${block.workchain},${block.shard},${block.seqno},${rootHash},${fileHash})`,
  }
}

function blockUnixTime(block: V3Block): number | undefined {
  const value = Number(block.gen_utime)
  return Number.isFinite(value) && value > 0 ? value : undefined
}

function formatAbsoluteBlockTime(block: V3Block): string {
  const unixTime = blockUnixTime(block)
  if (unixTime === undefined) {
    return "Unknown"
  }

  const date = new Date(unixTime * 1000)
  const day = date.getDate().toString().padStart(2, "0")
  const month = (date.getMonth() + 1).toString().padStart(2, "0")
  const hours = date.getHours().toString().padStart(2, "0")
  const minutes = date.getMinutes().toString().padStart(2, "0")
  const seconds = date.getSeconds().toString().padStart(2, "0")
  return `${day}.${month}.${date.getFullYear()}, ${hours}:${minutes}:${seconds}`
}

function formatTransactionExitCode(transaction: V3TransactionListItem): string {
  const computeExitCode = transaction.description.compute_ph?.exit_code
  if (typeof computeExitCode === "number") {
    return computeExitCode.toString()
  }
  const resultCode = transaction.description.action?.result_code
  return typeof resultCode === "number" ? resultCode.toString() : "Unknown"
}

function compactMiddle(value: string, visibleChars: number): string {
  if (value.length <= visibleChars + 3) {
    return value
  }

  const side = Math.max(4, Math.floor(visibleChars / 2))
  return `${value.slice(0, side)}…${value.slice(-side)}`
}
