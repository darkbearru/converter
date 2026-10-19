import { mountApplication } from './shared/bootstrap';
import { nativeBridge } from './shared/native';
import './styles/tokens.css';
import './styles/app.css';
void mountApplication(nativeBridge);
