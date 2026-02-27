import { useState } from 'react';
import { Dashboard } from './components/Dashboard';
import { TodoList } from './components/TodoList';
import './styles/index.css';

type Tab = 'items' | 'todos';

function App() {
  const [activeTab, setActiveTab] = useState<Tab>('items');

  return (
    <div className="container">
      <div className="tab-bar">
        <button
          className={`tab-btn ${activeTab === 'items' ? 'active' : ''}`}
          onClick={() => setActiveTab('items')}
        >
          Items
        </button>
        <button
          className={`tab-btn ${activeTab === 'todos' ? 'active' : ''}`}
          onClick={() => setActiveTab('todos')}
        >
          Todos
        </button>
      </div>
      {activeTab === 'items' ? <Dashboard /> : <TodoList />}
    </div>
  );
}

export default App;
