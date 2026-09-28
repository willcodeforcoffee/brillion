import { ApolloClient, HttpLink, InMemoryCache } from '@apollo/client'
import { SetContextLink } from '@apollo/client/link/context'
import { API_BASE } from './config'
import { getAccessToken } from './auth'

const authLink = new SetContextLink((prevContext) => {
  const token = getAccessToken()
  return token
    ? { headers: { ...prevContext.headers, Authorization: `Bearer ${token}` } }
    : {}
})

const httpLink = new HttpLink({ uri: `${API_BASE}/graphql` })

export const apolloClient = new ApolloClient({
  link: authLink.concat(httpLink),
  cache: new InMemoryCache(),
})
